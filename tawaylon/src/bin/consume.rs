use memfile::{CreateOptions, MemFile};
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
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
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

use tawaylon::keymap::get_temp_keymap;

/// Generate a keycode for a character.
/// Our keymap maps keycodes as ASCII, so this is just a unity mapping.
fn make_keycode(c: u32) -> u32 {
    c
}

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
    shm_pool: Option<WlShmPool>,

    memfile: MemFile,
    keymap: KeymapInfo,

    state: InitKeyboardState,
    count: u32,
}

#[derive(Default, Debug)]
enum InitKeyboardState {
    #[default]
    Nothing,
    HaveKeyboard(ZwpVirtualKeyboardV1),
}

impl Dispatcher {
    fn start() -> VirtualKeyboard {
        // Set up Wayland end:
        let connection = Connection::connect_to_env().unwrap();
        let display = connection.display();
        let event_queue = connection.new_event_queue();
        let queue_handle = event_queue.handle();

        let (keypipe, waypipe) = mio::unix::pipe::new().unwrap();

        let keymap = Self::make_keymap_info();
        let memfile = CreateOptions::new()
            .create("wl_shm")
            .expect("could not create shared memory pool");
        memfile
            .set_len(4 * 1024 * 1024)
            .expect("could not resize pool");

        let dispatcher = Dispatcher {
            pipe: waypipe,
            queue_handle: queue_handle.clone(),
            seat: None,
            keyboard: None,
            keymap,
            memfile,
            shm_pool: None,
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
                tracing::debug!("flushing to Wayland");
                // Note! dispatch_pending handles _incoming_ requests,
                // but here we want to flush outbound requests.
                event_queue.flush().unwrap();
            }
            poll.registry().deregister(&mut fd).unwrap();
            if let Err(e) = read_guard.read() {
                tracing::warn!("error in handling read guard: {e}");
            }
            tracing::debug!("handling pending Wayland events");
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
            let mut bytes: [u8; 4] = [0; 4];
            match self.pipe.read_exact(&mut bytes) {
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    tracing::info!("Done processing data");
                    return;
                }
                Err(e) => panic!("unexpected error in reading input channel: {}", e),
                Ok(_) => (),
            }
            tracing::debug!("read 4 bytes from sender");
            let u = u32::from_ne_bytes(bytes);
            tracing::debug!("read character: {}", char::from_u32(u).unwrap());

            let code = make_keycode(u);
            if let InitKeyboardState::HaveKeyboard(kb) = &self.state {
                kb.keymap(
                    self.keymap.format.into(),
                    self.keymap.fd.as_fd(),
                    self.keymap.size,
                );
                kb.key(self.count, code, KeyState::Pressed.into());
                self.count += 100;
                kb.key(self.count, code, KeyState::Released.into());
                self.count += 100;
            } else {
                tracing::warn!("keyboard is not ready!");
            }
        }
    }

    fn add_kmm(&mut self, kmm: ZwpVirtualKeyboardManagerV1) {
        tracing::debug!("got keyboard manager {:?}", kmm);
        if self.kb_manager.is_none() {
            self.kb_manager = Some(kmm);
            self.init_keyboard();
        }
    }

    fn add_seat(&mut self, seat: WlSeat) {
        tracing::debug!("got seat {:?}", seat);
        if self.keyboard.is_none() {
            tracing::debug!("getting keyboard input");
            self.keyboard = Some(seat.get_keyboard(&self.queue_handle, ()));
        }
        if self.seat.is_none() {
            self.seat = Some(seat);
        }
        // Check if we fulfilled our last dependency
        self.init_keyboard();
    }

    fn add_shm(&mut self, shm: WlShm, qh: &QueueHandle<Self>) {
        tracing::debug!("got shared memory manager{:?}", shm);
        if self.shm_pool.is_none() {
            let len = self
                .memfile
                .metadata()
                .expect("could not get memfile metadata")
                .len();
            tracing::info!("sharing memory pool of size {}", len);
            // Create a pool to use for buffers
            self.shm_pool = Some(shm.create_pool(self.memfile.as_fd(), len as i32, qh, ()));
        }
        self.init_keyboard();
    }

    fn make_keymap_info() -> KeymapInfo {
        tracing::debug!("creating keymap file");
        let kmfile = get_temp_keymap().expect("failed to prepare keymap");
        let size = kmfile
            .metadata()
            .expect("could not key keymap metadata")
            .len() as u32;
        let fd: OwnedFd = kmfile.into();
        let info = KeymapInfo {
            format: KeymapFormat::XkbV1,
            fd,
            size,
        };

        tracing::debug!("generated keymap info {:?}", info);
        info
    }

    fn init_keyboard(&mut self) {
        tracing::debug!("reevaluating keyboard state");
        // Init the keymap if needed
        if let (Some(seat), Some(kb_manager), Some(_)) =
            (&self.seat, &self.kb_manager, &self.shm_pool)
        {
            tracing::info!("creating keyboard");
            let kb = kb_manager.create_virtual_keyboard(seat, &self.queue_handle, ());
            // Send a keymap identical to the one actually on the seat.
            // TODO: This fails with "no memory".
            // I wonder if we have to allocate e.g. a shared-memory pool first?
            // I've had this work when it's an FD already-shared in the host...
            // or "wl_shm::create_pool"?
            // TODO: add this back, working on SHM
            kb.keymap(
                self.keymap.format.into(),
                self.keymap.fd.as_fd(),
                self.keymap.size,
            );

            let mut newstate = InitKeyboardState::Nothing;
            std::mem::swap(&mut newstate, &mut self.state);
            self.state = InitKeyboardState::HaveKeyboard(kb);
            tracing::info!("READY!!!!");
        } else {
            tracing::warn!("not ready for keyboard: {:?}", self)
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for Dispatcher {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        event: <wl_shm::WlShm as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::debug!("got shared memory event: {:?}", event);
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for Dispatcher {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        event: <wl_shm_pool::WlShmPool as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::debug!("got shared memory pool event: {:?}", event);
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
        tracing::debug!("got seat event: {:?}", event);
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Dispatcher {
    fn event(
        _state: &mut Self,
        _proxy: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        tracing::debug!("got keyboard event: {:?}", event);
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
            tracing::debug!("got registry entry: {interface}");
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
                "wl_shm" => {
                    // Shared memory manager is a global singleton.
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(
                        name,
                        /*version=*/ 1,
                        qh,
                        /* udata=*/ (),
                    );
                    state.add_shm(shm, qh);
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
        // Assuming we're invoked from redo
        let model_path = "../models/vosk-small.dir";

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
                    tracing::debug!("got letter: {}", char::from_u32(c).unwrap());
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
