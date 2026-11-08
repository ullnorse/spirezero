//! `PowerCell` (`Models.Relics/PowerCell.cs`): before the owner's first turn, moves 2 random
//! zero-cost cards of the draw pile to the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CreatureId, Engine, Flow, PileType, RelicId, Side};
use crate::content::cards;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct PowerCell;

impl RelicModel for PowerCell {
    fn name(&self) -> &'static str {
        "Power Cell"
    }
}

#[sts2_macros::hooks]
impl Hooks for PowerCell {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&eng.s.player.creature) || eng.s.player.turn_number > 1 {
            return Ok(());
        }
        // `!CostsX && GetWithModifiers(CostModifiers.Local) == 0`: a negative base (unplayable)
        // is returned as is, and an X cost is excluded.
        let mut free: Vec<CardId> = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .filter(|&c| {
                let cost = &eng.s.card(c).energy_cost;
                !cost.costs_x && eng.s.local_energy_cost(c).is_ok_and(|n| n <= 0)
            })
            .collect();
        let state = &mut *eng.s;
        let all_cards = &state.cards;
        stable_shuffle_by(
            &mut free,
            &mut state.rng[RunRngType::CombatCardSelection],
            |a, b| cards::compare(&all_cards[a.index()], &all_cards[b.index()]),
        );
        free.truncate(CARDS);
        eng.add_cards_to_pile(&free, PileType::Hand, Position::Bottom)
    }
}
