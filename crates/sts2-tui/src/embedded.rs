//! The game text built into the binary by the `embedded` feature (`build.rs`), so a
//! copied binary needs no install and no data files.

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
