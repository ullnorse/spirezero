//! Test-only affliction: proves a new file registers without touching `mod.rs`
//! (`content::tests::new_files_register`).

#![cfg(test)]

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;

/// Test-only: registered from its file alone.
pub struct TestDummy;

impl AfflictionModel for TestDummy {
    fn name(&self) -> &'static str {
        "Test Dummy"
    }
}

#[sts2_macros::hooks]
impl Hooks for TestDummy {
    type Me = CardId;
}
