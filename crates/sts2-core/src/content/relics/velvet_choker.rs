//! `VelvetChoker` (`Models.Relics/VelvetChoker.cs`): +1 max energy; once the owner has played 6
//! cards this turn, no card can be played (`ShouldPlay`). The count resets on entering a combat
//! room, at combat end and before each of the owner's turns.

use crate::combat::AutoPlayType;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(6)`.
const CARDS: i32 = 6;
/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct VelvetChoker;

/// `VelvetChoker`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_cardsPlayedThisTurn`.
    pub cards_played_this_turn: i32,
}

impl RelicModel for VelvetChoker {
    fn name(&self) -> &'static str {
        "Velvet Choker"
    }

    fn init_data(&self) -> RelicData {
        RelicData::VelvetChoker(Data::default())
    }
}

/// The relic's `_cardsPlayedThisTurn`.
fn cards_played(s: &mut CombatState, me: RelicId) -> &mut i32 {
    let RelicData::VelvetChoker(data) = &mut s.relic_mut(me).data else {
        unreachable!("a VelvetChoker has VelvetChoker data");
    };
    &mut data.cards_played_this_turn
}

#[sts2_macros::hooks]
impl Hooks for VelvetChoker {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }

    fn should_play(
        &self,
        s: &CombatState,
        me: RelicId,
        _card: CardId,
        _auto_play: AutoPlayType,
    ) -> bool {
        // The only player owns every card.
        let RelicData::VelvetChoker(data) = s.relic(me).data else {
            unreachable!("a VelvetChoker has VelvetChoker data");
        };
        data.cards_played_this_turn < CARDS
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, _play: &CardPlay) -> Flow {
        *cards_played(eng.s, me) += 1;
        Ok(())
    }

    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        // Every combat's room is a `CombatRoom`.
        *cards_played(eng.s, me) = 0;
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *cards_played(eng.s, me) = 0;
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            *cards_played(eng.s, me) = 0;
        }
        Ok(())
    }
}
