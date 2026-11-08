//! `Quasar` (`Models.Cards/Quasar.cs`): Regent uncommon skill, cost 0 and 2 stars; choose 1 of 3
//! random colorless cards (upgraded if this card is) or none, and add it to the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, PromptSource};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType, pools};

pub struct Quasar;

#[sts2_macros::hooks]
impl Hooks for Quasar {
    type Me = CardId;
}

impl CardModel for Quasar {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Quasar"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let options = eng.get_distinct_for_combat(&pools::colorless(), 3)?;
        if eng.s.card(play.card).is_upgraded() {
            for &card in &options {
                eng.upgrade_card(card);
            }
        }
        if let Some(card) = eng.choose_a_card(PromptSource::Card(play.card), options)? {
            let player = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
