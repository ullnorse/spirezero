//! The integration tests of `sts2-tui`, one binary: every file here is a module (a binary per file
//! linked its own copy of the engine and the TUI). One old file: `cargo test -p sts2-tui --test it mouse`.
//! The tests share a process; `crash` installs a panic hook for it, which only matters if another test panics.

mod crash;
mod enchant_ascension;
mod full_run;
mod input;
mod mouse;
mod potions;
mod rooms;
mod session;
mod snapshots;
mod whole_game;
