//! `DivineDestiny` (`Models.Relics/DivineDestiny.cs`): Regent's upgraded starter relic
//! (`TouchOfOrobas`); gain 7 stars at the start of the owner's first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `StarsVar(7)`.
const STARS: Decimal = Decimal::new(7, 0);

pub struct DivineDestiny;

impl RelicModel for DivineDestiny {
    fn name(&self) -> &'static str {
        "Divine Destiny"
    }
}

#[sts2_macros::hooks]
impl Hooks for DivineDestiny {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number <= 1 {
            eng.gain_stars(STARS)?;
        }
        Ok(())
    }
}
