use rodio::{
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    cpal::{traits::StreamTrait, SampleRate},
    DeviceTrait,
};
use vosk::{CompleteResult, Model, Recognizer};

const SAMPLE_RATE: SampleRate = SampleRate(16_000);

struct Robot {
    recog: Recognizer,
}

impl Robot {
    fn new() -> Self {
        let model_path = "/home/cceckman/r/github.com/cceckman/tawaylon/models/vosk-small.dir";

        let model = Model::new(model_path).unwrap();

        let mut recognizer = Recognizer::new(&model, SAMPLE_RATE.0 as f32).unwrap();

        // recognizer.set_max_alternatives(10);
        recognizer.set_words(true);
        recognizer.set_partial_words(false);

        Robot { recog: recognizer }
    }

    fn update(&mut self, sample: &[i16], _: &InputCallbackInfo) {
        let state = self.recog.accept_waveform(sample);
        if let vosk::DecodingState::Finalized = state {
            if let CompleteResult::Single(result) = self.recog.final_result() {
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
    let mut robot = Box::new(Robot::new());

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
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("done!");
}
