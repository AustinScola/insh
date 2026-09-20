/*!
Turns text into vectors so that it can be searched for by meaning rather than by wording.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

use model2vec_rs::model::StaticModel;

/// The model which vectors are made with.
///
/// This is recorded alongside every vector, so that vectors made by a model which is no longer the
/// one in use can be told apart and made again.
pub const MODEL_ID: &str = "minishlab/potion-base-8M";

/// The number of numbers in a vector.
pub const DIMENSIONS: usize = 256;

/// The tokenizer of the model.
const TOKENIZER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tokenizer.json"));

/// The weights of the model.
const WEIGHTS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.safetensors"));

/// The configuration of the model.
const CONFIG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/config.json"));

/// Turns text into vectors.
pub struct Embedder {
    /// The model which vectors are made with.
    model: StaticModel,
}

impl Embedder {
    /// Return an embedder using the model which is embedded in the executable.
    pub fn load() -> Result<Self, LoadError> {
        let model: StaticModel = StaticModel::from_bytes(TOKENIZER, WEIGHTS, CONFIG, None)
            .map_err(|error| LoadError {
                error: error.to_string(),
            })?;

        return Ok(Self { model });
    }

    /// Return a vector for each of some texts.
    pub fn embed(&self, texts: &[String]) -> Vec<Vec<f32>> {
        return self.model.encode(texts);
    }

    /// Return the vector of one text.
    pub fn embed_one(&self, text: &str) -> Vec<f32> {
        return self.model.encode_single(text);
    }
}

mod load_error {
    //! An error loading the model.

    use std::fmt::{Display, Error as FmtError, Formatter};

    /// An error loading the model.
    #[derive(Debug)]
    pub struct LoadError {
        /// The error which was encountered.
        pub error: String,
    }

    impl Display for LoadError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            write!(formatter, "Failed to load the model: {}", self.error)
        }
    }
}
pub use load_error::LoadError;

#[cfg(test)]
mod tests {
    use super::*;

    /// Return the cosine similarity of two vectors.
    ///
    /// The model normalizes what it returns, so this is only the dot product.
    fn similarity(left: &[f32], right: &[f32]) -> f32 {
        return left
            .iter()
            .zip(right.iter())
            .map(|(left, right)| left * right)
            .sum();
    }

    #[test]
    fn test_dimensions() {
        let embedder: Embedder = Embedder::load().unwrap();

        assert_eq!(embedder.embed_one("hello").len(), DIMENSIONS);
    }

    #[test]
    fn test_embeds_every_text() {
        let embedder: Embedder = Embedder::load().unwrap();
        let texts: Vec<String> = vec!["one".to_string(), "two".to_string(), "three".to_string()];

        let vectors: Vec<Vec<f32>> = embedder.embed(&texts);

        assert_eq!(vectors.len(), texts.len());
        assert!(vectors.iter().all(|vector| vector.len() == DIMENSIONS));
    }

    #[test]
    fn test_paraphrases_are_closer_than_unrelated_texts() {
        let embedder: Embedder = Embedder::load().unwrap();

        let question: Vec<f32> = embedder.embed_one("how do I rename a file in the terminal");
        let paraphrase: Vec<f32> = embedder.embed_one("renaming files from the command line");
        let unrelated: Vec<f32> = embedder.embed_one("the price of apples at the market");

        assert!(
            similarity(&question, &paraphrase) > similarity(&question, &unrelated),
            "The paraphrase should be closer to the question than the unrelated text is."
        );
    }

    #[test]
    fn test_emoji() {
        let embedder: Embedder = Embedder::load().unwrap();

        assert_eq!(embedder.embed_one("🚀 launch").len(), DIMENSIONS);
    }
}
