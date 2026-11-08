//! Shared fixture loading for the golden-vector tests.

use serde::Deserialize;
use sts2_core::rng::{Rng, RngState};

/// Parses `tests/golden/<name>`.
pub fn load<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A `SerializableRng` as written by the generator.
#[derive(Debug, Deserialize)]
pub struct State {
    pub counter: i32,
    pub s: [u64; 4],
}

impl State {
    pub const fn to_state(&self) -> RngState {
        RngState {
            counter: self.counter,
            s: self.s,
        }
    }

    /// Asserts that `rng` is exactly in this state.
    #[track_caller]
    pub fn assert_matches(&self, rng: &Rng, context: &str) {
        assert_eq!(rng.state(), self.to_state(), "{context}");
    }
}
