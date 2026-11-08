//! `ArtOfWar` (`Models.Relics/ArtOfWar.cs`): 1 energy at the start of a turn (after the first)
//! if the owner played no Attack in the turn before.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct ArtOfWar;

/// `ArtOfWar`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AnyAttacksPlayedLastTurn`.
    pub any_attacks_played_last_turn: bool,
    /// `AnyAttacksPlayedThisTurn`.
    pub any_attacks_played_this_turn: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::ArtOfWar(data) = &mut s.relic_mut(me).data else {
        unreachable!("an ArtOfWar has ArtOfWar data");
    };
    data
}

impl RelicModel for ArtOfWar {
    fn name(&self) -> &'static str {
        "Art of War"
    }

    fn init_data(&self) -> RelicData {
        RelicData::ArtOfWar(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for ArtOfWar {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress || eng.s.card(play.card).card_type() != CardType::Attack {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        if !data.any_attacks_played_last_turn {
            data.any_attacks_played_this_turn = true;
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            let data = data_mut(eng.s, me);
            data.any_attacks_played_last_turn = data.any_attacks_played_this_turn;
            data.any_attacks_played_this_turn = false;
        }
        Ok(())
    }

    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        if eng.s.player.turn_number > 1 {
            if !data_mut(eng.s, me).any_attacks_played_last_turn {
                eng.gain_energy(ENERGY)?;
            }
            let data = data_mut(eng.s, me);
            data.any_attacks_played_last_turn = false;
            data.any_attacks_played_this_turn = false;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }
}
