//! `Anointed` (`Models.Cards/Anointed.cs`): cost 1, Exhaust; moves random Rare cards from the
//! draw pile to the hand until it is full (`TakeRandom` on `Rng.CombatCardSelection`). Upgrading
//! adds Retain.

use crate::combat::commands::MAX_HAND_SIZE;
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::rng::RunRngType;
use crate::shuffle::unstable_shuffle;

pub struct Anointed;

#[sts2_macros::hooks]
impl Hooks for Anointed {
    type Me = CardId;
}

impl CardModel for Anointed {
    fn name(&self) -> &'static str {
        "Anointed"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let count = MAX_HAND_SIZE.saturating_sub(eng.s.player.hand.len());
        let mut rares: Vec<CardId> = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).kind.rarity() == CardRarity::Rare)
            .collect();
        // `TakeRandom`: `UnstableShuffle` of the whole list, then the first `count`.
        unstable_shuffle(&mut rares, &mut eng.s.rng[RunRngType::CombatCardSelection]);
        rares.truncate(count);
        eng.add_cards_to_pile(&rares, PileType::Hand, Position::Bottom)
    }
}
