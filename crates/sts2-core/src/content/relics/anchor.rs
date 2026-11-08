//! `Anchor` (`Models.Relics/Anchor.cs`): 10 Block before combat starts.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(10m, ValueProp.Unpowered)`.
const BLOCK: i32 = 10;

pub struct Anchor;

impl RelicModel for Anchor {
    fn name(&self) -> &'static str {
        "Anchor"
    }
}

#[sts2_macros::hooks]
impl Hooks for Anchor {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
