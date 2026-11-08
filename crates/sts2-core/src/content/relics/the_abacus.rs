//! `TheAbacus` (`Models.Relics/TheAbacus.cs`): 6 Block whenever the owner shuffles.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(6m, ValueProp.Unpowered)`.
const BLOCK: i32 = 6;

pub struct TheAbacus;

impl RelicModel for TheAbacus {
    fn name(&self) -> &'static str {
        "The Abacus"
    }
}

#[sts2_macros::hooks]
impl Hooks for TheAbacus {
    type Me = RelicId;

    fn after_shuffle(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
