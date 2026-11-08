//! The integration tests of `sts2-diff`, one binary: every file here is a module (a binary per file
//! linked its own copy of the engine). One old file: `cargo test -p sts2-diff --test it fixtures`.

mod fixtures;
mod honest;
mod reach;
mod run_gen;
