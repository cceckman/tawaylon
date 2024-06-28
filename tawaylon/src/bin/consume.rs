use rodio::{
    cpal::traits::StreamTrait,
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    DeviceTrait,
};
use std::sync::mpsc::Receiver;
use std::{io::Write, time::Duration};
use tawaylon::{Recognizer, VirtualKeyboard, Word};

const SAMPLE_FREQUENCY: u32 = 16_000;

struct Robot {
    recog: Recognizer,
    keyboard: VirtualKeyboard,
    recv_word: Receiver<Word>,
}

impl Robot {
    fn new() -> Self {
        let keyboard = VirtualKeyboard::new();
        let grammar = keyboard.grammar();
        let (recognizer, recv_word) =
            Recognizer::new_with_grammar(SAMPLE_FREQUENCY as f32, grammar).unwrap();
        tracing::debug!("initialized recognizer");

        Robot {
            recog: recognizer,
            recv_word,
            keyboard,
        }
    }

    fn update(&mut self, sample: &[i16], cb_info: &InputCallbackInfo) {
        tracing::debug!("woke with received samples");
        let _ = cb_info.timestamp();
        self.recog.push_sample(sample);
        tracing::debug!("completed sample run");

        while let Ok(Word { word, .. }) = self.recv_word.try_recv() {
            let (idx, _) = self
                .keyboard
                .grammar()
                .enumerate()
                .find(|(_, v)| *v == word)
                .unwrap();
            let c: u32 = 'a' as u32 + idx as u32;
            tracing::debug!("got letter: {}", char::from_u32(c).unwrap());
            let b = c.to_le_bytes();
            self.keyboard.write_all(&b).unwrap();
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
        .with_sample_rate(rodio::cpal::SampleRate(SAMPLE_FREQUENCY))
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
