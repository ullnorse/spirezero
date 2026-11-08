//! `Pocketwatch` (`Models.Relics/Pocketwatch.cs`): draw 3 more cards at the start of a turn (after
//! the first) if the owner played 3 or fewer cards in the turn before.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `DynamicVar("CardThreshold", 3m)`.
const CARD_THRESHOLD: i32 = 3;
/// `CardsVar(3)`.
const CARDS: i32 = 3;

pub struct Pocketwatch;

/// `Pocketwatch`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_cardsPlayedThisTurn`.
    pub cards_played_this_turn: i32,
    /// `_cardsPlayedLastTurn`.
    pub cards_played_last_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Pocketwatch(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Pocketwatch has Pocketwatch data");
    };
    data
}

impl RelicModel for Pocketwatch {
    fn name(&self) -> &'static str {
        "Pocketwatch"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Pocketwatch(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Pocketwatch {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, _play: &CardPlay) -> Flow {
        if eng.s.in_progress {
            data_mut(eng.s, me).cards_played_this_turn += 1;
        }
        Ok(())
    }

    fn modify_hand_draw(&self, s: &CombatState, me: RelicId, count: Decimal) -> Decimal {
        let RelicData::Pocketwatch(data) = s.relic(me).data else {
            unreachable!("a Pocketwatch has Pocketwatch data");
        };
        if s.player.turn_number == 1 || data.cards_played_last_turn > CARD_THRESHOLD {
            return count;
        }
        count + Decimal::from(CARDS)
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            let data = data_mut(eng.s, me);
            data.cards_played_last_turn = data.cards_played_this_turn;
            data.cards_played_this_turn = 0;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }
}
