//! `Prophesize` (`Models.Cards/Prophesize.cs`): Regent uncommon skill, cost 2; draw 6 cards (9
//! upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Prophesize;

#[sts2_macros::hooks]
impl Hooks for Prophesize {
    type Me = CardId;
}

impl CardModel for Prophesize {
    fn name(&self) -> &'static str {
        "Prophesize"
    }

    fn cost(&self) -> i32 {
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.draw(cards, false)?;
        Ok(())
    }
}
