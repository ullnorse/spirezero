//! `WhiteNoise` (`Models.Cards/WhiteNoise.cs`): Defect uncommon skill, cost 1 (0 upgraded),
//! Exhaust; add a random Power of the owner's pool to the hand (`CombatCardGeneration`), free
//! this turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType, pools,
};

pub struct WhiteNoise;

#[sts2_macros::hooks]
impl Hooks for WhiteNoise {
    type Me = CardId;
}

impl CardModel for WhiteNoise {
    fn name(&self) -> &'static str {
        "White Noise"
    }

    fn cost(&self) -> i32 {
        1
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let powers: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Power)
            .collect();
        let picked = eng.get_distinct_for_combat(&powers, 1)?;
        if let Some(&card) = picked.first() {
            eng.s.set_to_free_this_turn(card);
            let player = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
