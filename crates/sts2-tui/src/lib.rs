//! Terminal UI for playing a whole run of any character on the engine.
//!
//! [`app::App`] holds all UI state and turns key presses into [`Run`](sts2_core::run::Run)
//! actions; [`ui`] only draws it. Neither contains game logic: legality comes from
//! `Run::legal_actions` and the screens are chosen by `Run::pending_decision`.

pub mod app;
pub use sts2_view::{describe, menu, names};
#[cfg(feature = "embedded")]
pub mod embedded;
pub mod hint;
pub mod hit;
pub mod keys;
pub mod session;
pub mod settings;
pub mod shell;
pub mod text;
pub mod ui;
