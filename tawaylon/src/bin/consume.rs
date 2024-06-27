use rodio::{
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    cpal::{traits::StreamTrait, SampleRate},
    DeviceTrait,
};
use std::{io::Write, time::Duration};
use tawaylon::VirtualKeyboard;
use vosk::{CompleteResult, Model, Recognizer};

const SAMPLE_RATE: SampleRate = SampleRate(16_000);

struct Robot {
    recog: Recognizer,
    keyboard: VirtualKeyboard,
}

impl Robot {
    fn new() -> Self {
        // Assuming we're invoked from redo
        let model_path = "../models/vosk-small.dir";

        let keyboard = VirtualKeyboard::new();
        let grammar: Vec<&str> = keyboard.grammar().collect();

        let mut model = Model::new(model_path).unwrap();
        for word in grammar.iter().clone() {
            if model.find_word(word.as_ref()).is_none() {
                panic!("word {} not found in the model", word)
            }
        }

        let mut recognizer =
            Recognizer::new_with_grammar(&model, SAMPLE_RATE.0 as f32, &grammar).unwrap();

        // recognizer.set_max_alternatives(10);
        recognizer.set_words(true);
        recognizer.set_partial_words(false);

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
                    let (idx, _) = self
                        .keyboard
                        .grammar()
                        .enumerate()
                        .find(|(_, v)| *v == word.word)
                        .unwrap();
                    let c: u32 = 'a' as u32 + idx as u32;
                    tracing::debug!("got letter: {}", char::from_u32(c).unwrap());
                    let b = c.to_le_bytes();
                    self.keyboard.write_all(&b).unwrap();
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
    let mut robot = Box::new(Robot::new());

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
