//! `RoyalGamble` (`Models.Cards/RoyalGamble.cs`): Regent uncommon skill, cost 0 and 5 stars, Exhaust; gain 9 stars. Upgrade adds Retain.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct RoyalGamble;

#[sts2_macros::hooks]
impl Hooks for RoyalGamble {
    type Me = CardId;
}

impl CardModel for RoyalGamble {
    fn name(&self) -> &'static str {
        "Royal Gamble"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
        5
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
        card.keywords.insert(CardKeyword::Retain);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Stars, 9)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))?;
        Ok(())
    }
}
