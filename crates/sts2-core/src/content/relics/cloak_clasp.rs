//! `CloakClasp` (`Models.Relics/CloakClasp.cs`): 1 Block per card in the hand before the player's
//! turn ends.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(1m, ValueProp.Unpowered)`.
const BLOCK: i32 = 1;

pub struct CloakClasp;

impl RelicModel for CloakClasp {
    fn name(&self) -> &'static str {
        "Cloak Clasp"
    }
}

#[sts2_macros::hooks]
impl Hooks for CloakClasp {
    type Me = RelicId;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        let cards = eng.s.player.hand.len();
        if participants.contains(&owner) && cards != 0 {
            // (int)(Cards.Count * Block.BaseValue).
            let amount = i32::try_from(cards).expect("a hand fits in i32") * BLOCK;
            eng.gain_block(owner, Decimal::from(amount), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
