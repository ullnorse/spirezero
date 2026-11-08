//! `ForgottenRitual` (`Models.Cards/ForgottenRitual.cs`): cost 1, Exhaust, gains 3 energy (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct ForgottenRitual;

#[sts2_macros::hooks]
impl Hooks for ForgottenRitual {
    type Me = CardId;
}

impl CardModel for ForgottenRitual {
    fn name(&self) -> &'static str {
        "Forgotten Ritual"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_energy(eng.s.card(play.card).var(Var::Energy))
    }
}
