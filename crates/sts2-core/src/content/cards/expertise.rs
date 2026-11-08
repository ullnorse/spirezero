//! `Expertise` (`Models.Cards/Expertise.cs`): cost 1, Skill, Uncommon; draws 2 cards (3
//! upgraded) and gives each drawn card single-turn Retain (`CardCmd.ApplySingleTurnRetain`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Expertise;

#[sts2_macros::hooks]
impl Hooks for Expertise {
    type Me = CardId;
}

impl CardModel for Expertise {
    fn name(&self) -> &'static str {
        "Expertise"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        for card in eng.draw(Decimal::from(cards), false)? {
            eng.apply_single_turn_retain(card);
        }
        Ok(())
    }
}
