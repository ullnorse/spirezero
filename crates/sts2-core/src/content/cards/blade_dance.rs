//! `BladeDance` (`Models.Cards/BladeDance.cs`): cost 1, Skill, Common, Exhaust; creates 3 Shivs
//! in the hand (4 upgraded), one `Shiv.CreateInHand` each.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::shiv;
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct BladeDance;

#[sts2_macros::hooks]
impl Hooks for BladeDance {
    type Me = CardId;
}

impl CardModel for BladeDance {
    fn name(&self) -> &'static str {
        "Blade Dance"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
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
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        for _ in 0..cards {
            shiv::create_in_hand(eng, 1)?;
        }
        Ok(())
    }
}
