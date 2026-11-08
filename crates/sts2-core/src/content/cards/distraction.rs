//! `Distraction` (`Models.Cards/Distraction.cs`): Event skill, cost 1 (0 upgraded), Exhaust; add
//! a random Skill of the owner's pool to the hand (`CombatCardGeneration`), free this turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType, pools,
};

pub struct Distraction;

#[sts2_macros::hooks]
impl Hooks for Distraction {
    type Me = CardId;
}

impl CardModel for Distraction {
    fn name(&self) -> &'static str {
        "Distraction"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let skills: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Skill)
            .collect();
        let picked = eng.get_distinct_for_combat(&skills, 1)?;
        if let Some(&card) = picked.first() {
            eng.s.set_to_free_this_turn(card);
            let player = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
