//! Support for recognizing words.

use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use vosk::{CompleteResult, Model};
mod vosk_models;

pub struct Recognizer {
    vosk: vosk::Recognizer,
    send_word: Sender<Word>,
}

// TODO: Use a string intern table; recognize by word ID.
pub struct Word {
    pub word: String,
    pub start: f32,
}

impl Recognizer {
    /// Add a new audio sample to this recognizer.
    pub fn push_sample(&mut self, sample: &[i16]) {
        tracing::debug!("sending samples to vosk");
        let state = self.vosk.accept_waveform(sample);
        tracing::debug!("vosk completed processing");
        if let vosk::DecodingState::Finalized = state {
            if let CompleteResult::Single(result) = self.vosk.final_result() {
                tracing::info!("got utterance: {}", result.text);

                // TODO: Use a string intern table, recognize by ID
                for word in result.result {
                    let _ = self.send_word.send(Word {
                        word: word.word.to_owned(),
                        start: word.start,
                    });
                }
            } else {
                panic!("multiple results")
            }
        }
        tracing::debug!("processed some samples")
    }

    pub fn new_with_grammar<'a>(
        sample_frequency: f32,
        grammar: impl Iterator<Item = &'a str>,
    ) -> Result<(Recognizer, Receiver<Word>), String> {
        let model_path = vosk_models::get()?;
        let model_dir = model_path.as_path().to_str().unwrap();
        let grammar: Vec<&str> = grammar.collect();

        let mut model = Model::new(model_dir).unwrap();
        for word in grammar.iter().clone() {
            if model.find_word(word.as_ref()).is_none() {
                panic!("word {} not found in the model", word)
            }
        }

        let mut recognizer =
            vosk::Recognizer::new_with_grammar(&model, sample_frequency, &grammar).unwrap();

        // recognizer.set_max_alternatives(10);
        recognizer.set_words(true);
        recognizer.set_partial_words(false);

        let (send_word, recv_word) = std::sync::mpsc::channel();

        Ok((
            Recognizer {
                vosk: recognizer,
                send_word,
            },
            recv_word,
        ))
    }
}
