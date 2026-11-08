//! `Stoke` (`Models.Cards/Stoke.cs`): exhaust the whole hand, then add that many random cards of
//! the owner's pool to the hand (upgraded, each of them is upgraded).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType, pools};

pub struct Stoke;

#[sts2_macros::hooks]
impl Hooks for Stoke {
    type Me = CardId;
}

impl CardModel for Stoke {
    fn name(&self) -> &'static str {
        "Stoke"
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

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let hand = eng.s.player.hand.to_vec();
        for &card in &hand {
            eng.exhaust(card, false)?;
        }
        // CardFactory.GetForCombat(.., Owner.Character.CardPool.GetUnlockedCards(..), exhaustCount,
        // Rng.CombatCardGeneration).
        let pool = pools::character(eng.s.player.character);
        let cards = pools::get_for_combat(eng, &pool, hand.len())?;
        if eng.s.card(play.card).is_upgraded() {
            for &card in &cards {
                eng.upgrade_card(card);
            }
        }
        let owner = eng.s.player.creature;
        eng.add_generated_cards_to_combat(&cards, PileType::Hand, Some(owner), Position::Bottom)
    }
}
