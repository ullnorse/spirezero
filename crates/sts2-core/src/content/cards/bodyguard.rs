//! `Bodyguard` (`Models.Cards/Bodyguard.cs`): Summon 5, upgraded 7.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Bodyguard;

#[sts2_macros::hooks]
impl Hooks for Bodyguard {
    type Me = CardId;
}

impl CardModel for Bodyguard {
    fn name(&self) -> &'static str {
        "Bodyguard"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Summon, 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.summon_osty(eng.s.card(play.card).var(Var::Summon))
    }
}
