//! `BoomingConch` (`Models.Relics/BoomingConch.cs`): in elite rooms, 2 more cards in the first
//! hand and 1 energy at the start of the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RoomType;

/// `CardsVar(2)`.
const CARDS: i32 = 2;
/// `EnergyVar(1)`.
const ENERGY: i32 = 1;

pub struct BoomingConch;

impl RelicModel for BoomingConch {
    fn name(&self) -> &'static str {
        "Booming Conch"
    }
}

#[sts2_macros::hooks]
impl Hooks for BoomingConch {
    type Me = RelicId;

    fn modify_hand_draw(&self, s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        // `player != Owner`: the only player draws.
        if s.player.turn_number > 1 || s.room_type != RoomType::Elite {
            return count;
        }
        count + Decimal::from(CARDS)
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature)
            && eng.s.player.turn_number <= 1
            && eng.s.room_type == RoomType::Elite
        {
            eng.gain_energy(Decimal::from(ENERGY))?;
        }
        Ok(())
    }
}
