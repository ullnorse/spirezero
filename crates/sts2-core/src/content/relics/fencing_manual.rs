//! `FencingManual` (`Models.Relics/FencingManual.cs`): Forge 10 at the start of the owner's
//! side's first turn.

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `ForgeVar(10)`.
const FORGE: i32 = 10;

pub struct FencingManual;

impl RelicModel for FencingManual {
    fn name(&self) -> &'static str {
        "Fencing Manual"
    }
}

#[sts2_macros::hooks]
impl Hooks for FencingManual {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number <= 1 {
            eng.forge(Decimal::from(FORGE), Some(Listener::Relic(me)))?;
        }
        Ok(())
    }
}
