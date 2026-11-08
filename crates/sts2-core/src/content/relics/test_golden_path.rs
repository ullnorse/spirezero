//! Test-only relic for `ModifyGeneratedMap` and regenerating the map after an obtain, shaped like
//! `GoldenCompass` (`Models.Relics/GoldenCompass.cs`) without its act field: every act's map is
//! the `GoldenPathActMap`, and obtaining it regenerates the current map.

#![cfg(test)]

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::map::{ActMap, MapError};
use crate::run::{RunCtx, RunError, RunState};

/// Test-only: `ModifyGeneratedMap` and `GenerateMap` after an obtain.
pub struct TestGoldenPath;

impl RelicModel for TestGoldenPath {
    fn name(&self) -> &'static str {
        "Test Golden Path"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.regenerate_map()
    }

    fn modify_generated_map(
        &self,
        _s: &mut RunState,
        _me: usize,
        map: &mut ActMap,
        _act_index: usize,
    ) -> Result<(), MapError> {
        *map = ActMap::golden_path();
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for TestGoldenPath {
    type Me = RelicId;
}
