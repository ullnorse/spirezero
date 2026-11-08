//! `Skim` (`Models.Cards/Skim.cs`): Defect uncommon skill, cost 1; draw 3 cards (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Skim;

#[sts2_macros::hooks]
impl Hooks for Skim {
    type Me = CardId;
}

impl CardModel for Skim {
    fn name(&self) -> &'static str {
        "Skim"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
