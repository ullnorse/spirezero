//! `Akabeko` (`Models.Relics/Akabeko.cs`): 8 Vigor at the start of the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<VigorPower>(8m)` (`DynamicVars["VigorPower"].IntValue`).
const VIGOR: i32 = 8;

pub struct Akabeko;

impl RelicModel for Akabeko {
    fn name(&self) -> &'static str {
        "Akabeko"
    }
}

#[sts2_macros::hooks]
impl Hooks for Akabeko {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number <= 1 {
            eng.apply_power(owner, PowerKind::Vigor, Decimal::from(VIGOR), Some(owner))?;
        }
        Ok(())
    }
}
