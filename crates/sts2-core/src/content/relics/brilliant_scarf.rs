//! `BrilliantScarf` (`Models.Relics/BrilliantScarf.cs`): Ancient (Nonupeipe); the 5th card the
//! owner plays each turn costs 0 energy and 0 stars (`TryModifyEnergyCostInCombatLate`,
//! `TryModifyStarCost`, while `CardsPlayedThisTurn` is 4 and the card is in hand or being
//! played). Auto-plays do not count. The count resets at the owner's turn start and combat end.

use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType, RelicId, Side,
};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(5)`.
const CARDS: i32 = 5;

pub struct BrilliantScarf;

/// `BrilliantScarf`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CardsPlayedThisTurn`.
    pub cards_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::BrilliantScarf(data) = &mut s.relic_mut(me).data else {
        unreachable!("a BrilliantScarf has BrilliantScarf data");
    };
    data
}

/// `ShouldModifyCost`.
fn should_modify_cost(s: &CombatState, me: RelicId, card: CardId) -> bool {
    let RelicData::BrilliantScarf(data) = s.relic(me).data else {
        unreachable!("a BrilliantScarf has BrilliantScarf data");
    };
    s.in_progress
        && data.cards_played_this_turn == CARDS - 1
        && matches!(s.pile_of(card), Some(PileType::Hand | PileType::Play))
}

impl RelicModel for BrilliantScarf {
    fn name(&self) -> &'static str {
        "Brilliant Scarf"
    }

    fn init_data(&self) -> RelicData {
        RelicData::BrilliantScarf(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for BrilliantScarf {
    type Me = RelicId;

    fn try_modify_energy_cost_in_combat_late(
        &self,
        s: &CombatState,
        me: RelicId,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        should_modify_cost(s, me, card).then_some(Decimal::ZERO)
    }

    fn try_modify_star_cost(
        &self,
        s: &CombatState,
        me: RelicId,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        should_modify_cost(s, me, card).then_some(Decimal::ZERO)
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            data_mut(eng.s, me).cards_played_this_turn = 0;
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if eng.s.in_progress && !play.is_auto_play {
            data_mut(eng.s, me).cards_played_this_turn += 1;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).cards_played_this_turn = 0;
        Ok(())
    }
}
