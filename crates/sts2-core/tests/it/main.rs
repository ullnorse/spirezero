//! The integration tests of `sts2-core`, one binary: every file here is a module. A binary per file
//! linked its own copy of the crate (25 to 97 MB each, in every worktree's `target/`).
//!
//! Run one old file with a filter: `cargo test -p sts2-core --test it golden_rng`. The tests of a
//! binary share a process, which matters for the panic hook (`fuzz_combats` replaces it while it
//! runs, so a test that fails in that window loses its panic message).

mod combat_c2;
mod combat_m0;
mod common;
mod copy_alloc;
mod decimal;
mod fuzz_combats;
#[allow(
    clippy::float_arithmetic,
    reason = "checks the RNG's f32/f64 outputs bit for bit"
)]
#[allow(
    clippy::suboptimal_flops,
    reason = "the game computes `a * b + c` unfused; `mul_add` would change the last bit"
)]
mod golden_rng;
mod golden_seed_hash;
mod golden_shuffle;
mod preview;
