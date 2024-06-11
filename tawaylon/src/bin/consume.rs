use rodio::{
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    cpal::{traits::StreamTrait, SampleRate},
    DeviceTrait,
};
use std::sync::Arc;
use std::{sync::atomic::AtomicBool, time::Duration};
use vosk::{CompleteResult, Model, Recognizer};
use wayland_client::QueueHandle;
use wayland_client::{protocol::*, Connection};
use wayland_client::{Dispatch, EventQueue};
use wayland_protocols::wp::idle_inhibit::zv1::client::*;

const SAMPLE_RATE: SampleRate = SampleRate(16_000);

const GRAMMAR: &[&str] = &["air", "bat", "cap", "wake", "sleep", "click"];

// Wholeheartedly from https://github.com/mora-unie-youer/wayland-idle-inhibitor/blob/master/src/daemon/state.rs
struct Insomniac {
    pub terminate: Arc<AtomicBool>,
    queue_handle: QueueHandle<Self>,

    base_surface: Option<wl_surface::WlSurface>,
    idle_inhibit_manager: Option<zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1>,
    idle_inhibitor: Option<zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1>,
}

impl Insomniac {
    pub fn new(event_queue: &mut EventQueue<Self>) -> Self {
        let mut state = Self {
            terminate: Arc::new(AtomicBool::new(false)),
            queue_handle: event_queue.handle(),

            base_surface: None,
            idle_inhibit_manager: None,
            idle_inhibitor: None,
        };

        // Initializing Wayland client
        event_queue.roundtrip(&mut state).unwrap();
        state
    }

    pub fn create_idle_inhibitor(&self) -> zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1 {
        if self.base_surface.is_none() || self.idle_inhibit_manager.is_none() {
            panic!("Surface and idle inhibit manager were not initialized");
        }

        let surface = self.base_surface.as_ref().unwrap();
        let idle_inhibit_manager = self.idle_inhibit_manager.as_ref().unwrap();
        idle_inhibit_manager.create_inhibitor(surface, &self.queue_handle, ())
    }

    pub fn toggle_idle_inhibit(&mut self) {
        if let Some(idle_inhibitor) = &self.idle_inhibitor {
            idle_inhibitor.destroy();
            self.idle_inhibitor = None;
        } else {
            self.idle_inhibitor = Some(self.create_idle_inhibitor());
        }
    }

    pub fn enable_idle_inhibit(&mut self) {
        if self.idle_inhibitor.is_none() {
            self.idle_inhibitor = Some(self.create_idle_inhibitor());
        }
    }

    pub fn disable_idle_inhibit(&mut self) {
        if let Some(idle_inhibitor) = &self.idle_inhibitor {
            idle_inhibitor.destroy();
            self.idle_inhibitor = None;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.idle_inhibitor.is_some()
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for Insomniac {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        todo!()
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for Insomniac {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        todo!()
    }
}

impl Dispatch<zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1, ()> for Insomniac {
    fn event(
        _: &mut Self,
        _: &zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1,
        _: zwp_idle_inhibit_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        todo!()
    }
}

impl Dispatch<zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1, ()> for Insomniac {
    fn event(
        _: &mut Self,
        _: &zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
        _: zwp_idle_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        todo!()
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Insomniac {
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
            match &interface[..] {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<wl_compositor::WlCompositor, _, _>(name, 1, qh, ());
                    let surface = compositor.create_surface(qh, ());
                    state.base_surface = Some(surface);
                }
                "zwp_idle_inhibit_manager_v1" => {
                    state.idle_inhibit_manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

struct Robot {
    recog: Recognizer,
    event_queue: EventQueue<Insomniac>,
    insomniac: Insomniac,
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

        // Set up Wayland end:
        let conn = Connection::connect_to_env().unwrap();
        let display = conn.display();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();
        let _registry = display.get_registry(&qh, ());
        let insomniac = Insomniac::new(&mut event_queue);

        Robot {
            recog: recognizer,
            event_queue,
            insomniac,
        }
    }

    fn update(&mut self, sample: &[i16], _: &InputCallbackInfo) {
        let state = self.recog.accept_waveform(sample);
        if let vosk::DecodingState::Finalized = state {
            if let CompleteResult::Single(result) = self.recog.final_result() {
                if result.text == "wake" {
                    self.insomniac.enable_idle_inhibit()
                } else if result.text == "sleep" {
                    self.insomniac.disable_idle_inhibit()
                }
                println!("I heard:\n{}\n", result.text);
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
    println!("got error: {}", err);
}

fn main() {
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

    println!("starting input stream...");
    instream.play().unwrap();

    std::thread::sleep(Duration::from_secs(60));
}
