use mio::{Interest, Token};
use rodio::{
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    cpal::{traits::StreamTrait, SampleRate},
    DeviceTrait,
};
use std::{os::fd::AsRawFd, time::Duration};
use vosk::{CompleteResult, Model, Recognizer};
use wayland_client::{protocol::*, Connection};
use wayland_client::{
    protocol::{wl_display::WlDisplay, wl_seat::WlSeat},
    QueueHandle,
};
use wayland_client::{Dispatch, EventQueue};
use wlroots_extra_protocols::virtual_keyboard::v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, *,
};

const SAMPLE_RATE: SampleRate = SampleRate(16_000);

const GRAMMAR: &[&str] = &[
    "air", "bat", "cap", "drum", "each", "fine", "gust", "harp", "sit", "jury",
    /* nit: they don't have "krunch" in their model, we have to misspell it */ "crunch",
    "look", "made", "near", "odd", "pit", "quench", "red", "sun", "trap", "urge", "vest", "whale",
    "plex", "yank", "zip", "wake", "sleep", "click",
];

struct VirtualKeyboard {
    queue_handle: QueueHandle<Dispatcher>,

    // Shut down the event loop.
    cancel: Box<dyn Send + FnOnce()>,
}

impl VirtualKeyboard {
    pub fn new() -> Self {
        Dispatcher::start()
    }
}

impl Drop for VirtualKeyboard {
    fn drop(&mut self) {
        let mut cancel: Box<dyn Send + FnOnce()> = Box::new(|| {});
        std::mem::swap(&mut cancel, &mut self.cancel);
        cancel()
    }
}

/// Wayland dispatcher.
struct Dispatcher {}

impl Dispatcher {
    fn start() -> VirtualKeyboard {
        // Set up Wayland end:
        let connection = Connection::connect_to_env().unwrap();
        let display = connection.display();
        let event_queue = connection.new_event_queue();
        let queue_handle = event_queue.handle();
        let cancel =
            Box::new(|| tracing::error!("cancellation of wayland loop is not implemented"));

        let dispatcher = Dispatcher {};

        std::thread::spawn(move || dispatcher.run_loop(event_queue, connection, display));
        VirtualKeyboard {
            queue_handle,
            cancel,
        }
    }

    pub fn run_loop(
        mut self,
        mut event_queue: EventQueue<Self>,
        _connection: Connection,
        display: WlDisplay,
    ) {
        // Sync updates from Wayland and updates from ourselves.
        let mut poll = mio::Poll::new().unwrap();
        // TODO: Create a "wake" event we can use to shut things down.
        const SHUTDOWN: Token = Token(0);
        const WAYLAND: Token = Token(1);

        let queue_handle = event_queue.handle();
        // Invoke the registry to get global events, kick things off.
        let _registry = display.get_registry(&queue_handle, ());

        // ...and loop!
        loop {
            event_queue.flush().unwrap();

            let read_guard = event_queue.prepare_read().unwrap();
            let raw_read_guard = read_guard.connection_fd().as_raw_fd();
            let mut fd = mio::unix::SourceFd(&raw_read_guard);
            poll.registry()
                .register(&mut fd, WAYLAND, Interest::READABLE)
                .unwrap();

            // Wait until Wayland socket is ready...
            'wayland_wait: loop {
                let mut events = mio::Events::with_capacity(4);
                poll.poll(&mut events, Some(Duration::from_secs(1)))
                    .unwrap();
                for event in events.iter() {
                    match event.token() {
                        SHUTDOWN => return,
                        WAYLAND => break 'wayland_wait,
                        _ => {}
                    }
                }
            }
            poll.registry().deregister(&mut fd).unwrap();
            read_guard.read().unwrap();
            event_queue.dispatch_pending(&mut self).unwrap();
        }
    }

    fn add_seat(&mut self, seat: WlSeat) {
        tracing::info!("got seat {:?}", seat);
    }
    fn add_kmm(&mut self, kmm: ZwpVirtualKeyboardManagerV1) {
        tracing::info!("got keyboard manager {:?}", kmm);
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Dispatcher {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::info!("got seat event: {:?}", event);
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for Dispatcher {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        event: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::info!("got keyboard manager event: {:?}", event);
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Dispatcher {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            // tracing::info!("got global wl_registry event: {} {}", name, interface);
            match &interface[..] {
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(
                        name,
                        /*version=*/ 1,
                        qh,
                        /* udata=*/ (),
                    );
                    state.add_seat(seat);
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    let keyboardmanager = registry.bind::<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, _, _>(name, 1, qh, ());
                    state.add_kmm(keyboardmanager);
                }
                _ => {}
            }
        }
    }
}

struct Robot {
    recog: Recognizer,
    keyboard: VirtualKeyboard,
}

impl Robot {
    fn new(words: &[impl AsRef<str>]) -> Self {
        let model_path = "/home/cceckman/r/github.com/cceckman/tawaylon/models/vosk-small.dir";

        let mut model = Model::new(model_path).unwrap();
        for word in words {
            if model.find_word(word.as_ref()).is_none() {
                panic!("word {} not found in the model", word.as_ref())
            }
        }

        let mut recognizer =
            Recognizer::new_with_grammar(&model, SAMPLE_RATE.0 as f32, words).unwrap();

        // recognizer.set_max_alternatives(10);
        recognizer.set_words(true);
        recognizer.set_partial_words(false);

        let keyboard = VirtualKeyboard::new();

        Robot {
            recog: recognizer,
            keyboard,
        }
    }

    fn update(&mut self, sample: &[i16], _: &InputCallbackInfo) {
        let state = self.recog.accept_waveform(sample);
        if let vosk::DecodingState::Finalized = state {
            if let CompleteResult::Single(result) = self.recog.final_result() {
                tracing::info!("got utterance: {}", result.text);
            } else {
                panic!("multiple results")
            }
        }
    }
}

fn get_input_device() -> rodio::Device {
    default_host()
        .default_input_device()
        .expect("found no default input")
}

fn on_error(err: StreamError) {
    tracing::error!("got input stream error: {}", err)
}

fn main() {
    tracing_subscriber::fmt::init();

    let dev = get_input_device();
    let config = dev
        .supported_input_configs()
        .expect("no supported input configs")
        .find(|cfg| cfg.channels() == 1 && cfg.sample_format() == SampleFormat::I16)
        .expect("no desirable input configs")
        .with_sample_rate(SAMPLE_RATE)
        .config();
    let mut robot = Box::new(Robot::new(GRAMMAR));

    let instream = dev
        .build_input_stream(
            &config,
            move |data, info| robot.update(data, info),
            on_error,
            None,
        )
        .unwrap();

    instream.play().unwrap();

    std::thread::sleep(Duration::from_secs(60));
}
