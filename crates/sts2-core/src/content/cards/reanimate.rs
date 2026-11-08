//! `Reanimate` (`Models.Cards/Reanimate.cs`): cost 3, Exhaust, Summon 20 (25 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Reanimate;

#[sts2_macros::hooks]
impl Hooks for Reanimate {
    type Me = CardId;
}

impl CardModel for Reanimate {
    fn name(&self) -> &'static str {
        "Reanimate"
    }

    fn cost(&self) -> i32 {
        3
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Summon, 20).upgrade(5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.summon_osty(eng.s.card(play.card).var(Var::Summon))
    }
}
