//! `Venerate` (`Models.Cards/Venerate.cs`): Regent basic skill, cost 1; gain 2 stars (3
//! upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Venerate;

#[sts2_macros::hooks]
impl Hooks for Venerate {
    type Me = CardId;
}

impl CardModel for Venerate {
    fn name(&self) -> &'static str {
        "Venerate"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Stars, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))
    }
}
