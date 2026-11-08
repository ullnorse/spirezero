//! `BundleOfJoy` (`Models.Cards/BundleOfJoy.cs`): Regent rare skill, cost 1, Exhaust; add 3 (4
//! upgraded) distinct random colorless cards to the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var, pools,
};

pub struct BundleOfJoy;

#[sts2_macros::hooks]
impl Hooks for BundleOfJoy {
    type Me = CardId;
}

impl CardModel for BundleOfJoy {
    fn name(&self) -> &'static str {
        "Bundle of Joy"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let player = eng.s.player.creature;
        for card in eng.get_distinct_for_combat(&pools::colorless(), count)? {
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
