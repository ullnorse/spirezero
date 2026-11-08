//! `Alignment` (`Models.Cards/Alignment.cs`): Regent uncommon skill, cost 0 and 2 stars; gain 2 energy (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Alignment;

#[sts2_macros::hooks]
impl Hooks for Alignment {
    type Me = CardId;
}

impl CardModel for Alignment {
    fn name(&self) -> &'static str {
        "Alignment"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_energy(eng.s.card(play.card).var(Var::Energy))?;
        Ok(())
    }
}
