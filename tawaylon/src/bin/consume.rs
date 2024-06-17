use mio::{
    unix::pipe::{Receiver, Sender},
    Interest, Token,
};
use rodio::{
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    cpal::{traits::StreamTrait, SampleRate},
    DeviceTrait,
};
use std::{
    io::{ErrorKind, Read, Write},
    os::fd::{AsFd, AsRawFd, OwnedFd},
    time::Duration,
};
use vosk::{CompleteResult, Model, Recognizer};
use wayland_client::{
    protocol::{
        wl_display::WlDisplay,
        wl_keyboard::{KeymapFormat, WlKeyboard},
        wl_seat::WlSeat,
    },
    QueueHandle,
};
use wayland_client::{
    protocol::{wl_keyboard::KeyState, *},
    Connection,
};
use wayland_client::{Dispatch, EventQueue};
use wlroots_extra_protocols::virtual_keyboard::v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1, *,
};

const SAMPLE_RATE: SampleRate = SampleRate(16_000);

const GRAMMAR: &[&str] = &[
    "air", "bat", "cap", "drum", "each", "fine", "gust", "harp", "sit", "jury",
    /* nit: they don't have "krunch" in their model, we have to misspell it */ "crunch",
    "look", "made", "near", "odd", "pit", "quench", "red", "sun", "trap", "urge", "vest", "whale",
    "plex", "yank", "zip",
    // TODO: Beyond letters!
];

struct VirtualKeyboard {
    pipe: Sender,
}

/// Information reported from the compositor about a seat's keymap.
#[derive(Debug)]
struct KeymapInfo {
    format: KeymapFormat,
    fd: OwnedFd,
    size: u32,
}

impl VirtualKeyboard {
    pub fn new() -> Self {
        Dispatcher::start()
    }
}

/// Wayland dispatcher.
#[derive(Debug)]
struct Dispatcher {
    pipe: Receiver,
    queue_handle: QueueHandle<Self>,
    seat: Option<WlSeat>,
    keyboard: Option<WlKeyboard>,
    kb_manager: Option<ZwpVirtualKeyboardManagerV1>,

    state: InitKeyboardState,
    count: u32,
}

#[derive(Default, Debug)]
enum InitKeyboardState {
    #[default]
    Nothing,
    HaveKeymap(KeymapInfo),
    HaveKeyboard(ZwpVirtualKeyboardV1, KeymapInfo),
}

impl InitKeyboardState {
    fn needs_keymap(&self) -> bool {
        matches!(self, InitKeyboardState::Nothing)
    }
}

impl Dispatcher {
    fn start() -> VirtualKeyboard {
        // Set up Wayland end:
        let connection = Connection::connect_to_env().unwrap();
        let display = connection.display();
        let event_queue = connection.new_event_queue();
        let queue_handle = event_queue.handle();

        let (keypipe, waypipe) = mio::unix::pipe::new().unwrap();

        let dispatcher = Dispatcher {
            pipe: waypipe,
            queue_handle: queue_handle.clone(),
            seat: None,
            keyboard: None,
            kb_manager: None,
            state: Default::default(),
            count: 0,
        };

        std::thread::spawn(move || dispatcher.run_loop(event_queue, connection, display));
        VirtualKeyboard { pipe: keypipe }
    }

    pub fn run_loop(
        mut self,
        mut event_queue: EventQueue<Self>,
        _connection: Connection,
        display: WlDisplay,
    ) {
        // Sync updates from Wayland and updates from ourselves.
        let mut poll = mio::Poll::new().unwrap();
        const PIPE_DATA: Token = Token(0);
        const WAYLAND: Token = Token(2);
        poll.registry()
            .register(&mut self.pipe, PIPE_DATA, Interest::READABLE)
            .unwrap();
        self.pipe.set_nonblocking(true).unwrap();

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

            // Wait until one of the ports is ready...
            'wayland_wait: loop {
                let mut events = mio::Events::with_capacity(4);
                poll.poll(&mut events, None).unwrap();
                // Just try all of the events.
                for event in events.iter() {
                    if event.token() == WAYLAND {
                        tracing::info!("breaking for Wayland handling");
                        break 'wayland_wait;
                    }
                    if event.token() == PIPE_DATA {
                        tracing::info!("woke for input stream");
                        if event.is_read_closed() {
                            tracing::info!("input stream closed, shutting down");
                            return;
                        }
                    }
                }
                // Accept data even if there isn't any ready.
                self.accept_data();
                tracing::info!("dispatching to Wayland");
                event_queue.dispatch_pending(&mut self).unwrap();
            }
            poll.registry().deregister(&mut fd).unwrap();
            if let Err(e) = read_guard.read() {
                tracing::warn!("error in handling read guard: {e}");
            }
            tracing::info!("dispatching to Wayland");
            event_queue.dispatch_pending(&mut self).unwrap();
        }
    }

    fn accept_data(&mut self) {
        // TODO: It doesn't look like there's a great way to do level-triggered with Mio.
        // Need to rethink the "balancing" behavior some --
        // we don't want to get stuck processing only one event or the other.
        // For now, though, we do.
        loop {
            // Always transact a full codepoint.
            //
            // TODO: These events are going out to Wayland according to WAYLAND_DEBUG=client,
            // but they aren't _reliably_ being seen / consumed by other apps.
            // Sometimes they are! At startup, and some other random times?
            // We aren't seeing them reflected back into WlKeyboard,
            // but that's ~expected since we don't have a focused surface.
            let mut bytes: [u8; 4] = [0; 4];
            match self.pipe.read_exact(&mut bytes) {
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    tracing::info!("Done processing data");
                    return;
                }
                Err(e) => panic!("unexpected error in reading input channel: {}", e),
                Ok(_) => (),
            }
            tracing::info!("read 4 bytes from sender");
            let u = u32::from_ne_bytes(bytes);
            tracing::info!("read character: {}", char::from_u32(u).unwrap());

            // Empirically, this is the starting point:
            let code = u - ('a' as u32) + 30;
            if let InitKeyboardState::HaveKeyboard(kb, km) = &self.state {
                kb.keymap(km.format.into(), km.fd.as_fd(), km.size);
                kb.key(self.count, code, KeyState::Pressed.into());
                self.count += 10;
                kb.key(self.count, code, KeyState::Released.into());
                self.count += 10;
            } else {
                tracing::warn!("keyboard is not ready!");
            }
        }
    }

    fn add_kmm(&mut self, kmm: ZwpVirtualKeyboardManagerV1) {
        tracing::info!("got keyboard manager {:?}", kmm);
        if self.kb_manager.is_none() {
            self.kb_manager = Some(kmm);
            self.init_keyboard();
        }
    }

    fn add_seat(&mut self, seat: WlSeat) {
        tracing::info!("got seat {:?}", seat);
        if self.keyboard.is_none() {
            tracing::info!("getting keyboard input");
            self.keyboard = Some(seat.get_keyboard(&self.queue_handle, ()));
        }
        if self.seat.is_none() {
            self.seat = Some(seat);
        }
    }

    fn add_keymap_info(&mut self, info: KeymapInfo) {
        if self.state.needs_keymap() {
            tracing::info!("got new keymap info {:?}", info);
            self.state = InitKeyboardState::HaveKeymap(info);
            self.init_keyboard();
        }
    }

    fn init_keyboard(&mut self) {
        tracing::info!("reevaluating keyboard state");
        if let (Some(seat), Some(kb_manager), InitKeyboardState::HaveKeymap(km)) =
            (&self.seat, &self.kb_manager, &self.state)
        {
            tracing::info!("creating keyboard");
            let kb = kb_manager.create_virtual_keyboard(seat, &self.queue_handle, ());
            // Send a keymap identical to the one actually on the seat.
            kb.keymap(km.format.into(), km.fd.as_fd(), km.size);
            kb.key(self.count, 30, KeyState::Pressed.into());
            self.count += 1;
            kb.key(self.count, 30, KeyState::Released.into());
            self.count += 1;

            let mut newstate = InitKeyboardState::Nothing;
            std::mem::swap(&mut newstate, &mut self.state);
            let km = match newstate {
                InitKeyboardState::HaveKeymap(km) => km,
                _ => panic!("invalid state"),
            };
            self.state = InitKeyboardState::HaveKeyboard(kb, km);
        } else {
            tracing::info!("not ready for keyboard: {:?}", self)
        }
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
impl Dispatch<wl_keyboard::WlKeyboard, ()> for Dispatcher {
    fn event(
        state: &mut Self,
        _proxy: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::info!("got keyboard event: {:?}", event);
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                state.add_keymap_info(KeymapInfo {
                    format: format.into_result().unwrap(),
                    fd,
                    size,
                });
            }
            _ => tracing::debug!("ignored keyboard event"),
        }
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

impl Dispatch<ZwpVirtualKeyboardV1, ()> for Dispatcher {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        event: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::info!("got virtual keyboard event: {:?}", event);
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
                for word in result.result {
                    let (idx, _) = GRAMMAR
                        .iter()
                        .enumerate()
                        .find(|(_, v)| **v == word.word)
                        .unwrap();
                    let c: u32 = 'a' as u32 + idx as u32;
                    tracing::info!("got letter: {}", char::from_u32(c).unwrap());
                    let b = c.to_le_bytes();
                    self.keyboard.pipe.write_all(&b).unwrap();
                }
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
