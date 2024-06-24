use vosk::Model;
use vosk::Recognizer;

fn audio_samples() -> rodio::Decoder<std::io::Cursor<&'static [u8]>> {
    const SAMPLE_BYTES: &[u8] = include_bytes!("mono.wav");
    let cursor = std::io::Cursor::new(SAMPLE_BYTES);
    rodio::decoder::Decoder::new(cursor).unwrap()
}

fn main() {
    let samples: Vec<i16> = audio_samples().collect();
    let model_path = "../models/vosk-small.dir";

    let mut model = Model::new(model_path).unwrap();
    model.find_word("hello").expect("could not find hello");
    model.find_word("world").expect("could not find world");

    let mut recognizer = Recognizer::new(&model, 44100.0).unwrap();

    recognizer.set_max_alternatives(10);
    recognizer.set_words(true);
    recognizer.set_partial_words(false);

    recognizer.accept_waveform(&samples);
    println!("{:#?}", recognizer.final_result().multiple().unwrap());
}
