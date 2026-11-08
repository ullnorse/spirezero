//! `DiamondDiadem` (`Models.Relics/DiamondDiadem.cs`): Ancient (Nonupeipe); on the owner's first
//! turn (`TurnNumber <= 1`) gains 20 Block and 1 Blur at the start of the turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(20m, ValueProp.Unpowered)`.
const BLOCK: i32 = 20;

pub struct DiamondDiadem;

impl RelicModel for DiamondDiadem {
    fn name(&self) -> &'static str {
        "Diamond Diadem"
    }
}

#[sts2_macros::hooks]
impl Hooks for DiamondDiadem {
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
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
            eng.apply_power(owner, PowerKind::Blur, Decimal::ONE, Some(owner))?;
        }
        Ok(())
    }
}
