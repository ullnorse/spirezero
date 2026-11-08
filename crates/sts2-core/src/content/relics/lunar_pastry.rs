//! `LunarPastry` (`Models.Relics/LunarPastry.cs`): gain 1 star at the end of each of the
//! owner's turns.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `StarsVar(1)`.
const STARS: Decimal = Decimal::ONE;

pub struct LunarPastry;

impl RelicModel for LunarPastry {
    fn name(&self) -> &'static str {
        "Lunar Pastry"
    }
}

#[sts2_macros::hooks]
impl Hooks for LunarPastry {
    type Me = RelicId;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            eng.gain_stars(STARS)?;
        }
        Ok(())
    }
}
