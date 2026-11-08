//! Differential testing of the Rust engine against the referee (the real game, headless).
//!
//! Both sides speak the referee's action format and trace schema (`referee/README.md`); this
//! crate maps the engine's state into that schema, plays seeded random or greedy policies, and finds the
//! first step where the two traces disagree.

pub mod args;
pub mod capacity;
pub mod compare;
pub mod fields;
pub mod fixture;
pub mod greedy;
pub mod history;
pub mod history_replay;
pub mod hits;
pub mod honest;
pub mod ids;
pub mod loadout;
pub mod map;
pub mod policy;
pub mod reach;
pub mod run_session;
pub mod session;
pub mod sweep;

/// The error type for everything that can go wrong here; all failures are reported as text.
pub type Error = Box<dyn std::error::Error>;
/// `Result` with [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
