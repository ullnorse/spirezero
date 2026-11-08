//! `StoneCracker` (`Models.Relics/StoneCracker.cs`): at the start of a fight, upgrades 2 random
//! upgradable cards of the draw pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, RelicId};
use crate::content::cards;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct StoneCracker;

impl RelicModel for StoneCracker {
    fn name(&self) -> &'static str {
        "Stone Cracker"
    }
}

#[sts2_macros::hooks]
impl Hooks for StoneCracker {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room.
        let mut upgradable: Vec<CardId> = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).is_upgradable())
            .collect();
        let state = &mut *eng.s;
        let all_cards = &state.cards;
        stable_shuffle_by(
            &mut upgradable,
            &mut state.rng[RunRngType::CombatCardSelection],
            |a, b| cards::compare(&all_cards[a.index()], &all_cards[b.index()]),
        );
        for card in upgradable.into_iter().take(CARDS) {
            eng.upgrade_card(card);
        }
        Ok(())
    }
}
