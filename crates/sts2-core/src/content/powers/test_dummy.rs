//! Test-only power: proves a new file registers without touching `mod.rs`
//! (`content::tests::new_files_register`).

#![cfg(test)]

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

/// Test-only: registered from its file alone.
pub struct TestDummy;

impl PowerModel for TestDummy {
    fn name(&self) -> &'static str {
        "Test Dummy"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for TestDummy {
    type Me = PowerRef;
}
