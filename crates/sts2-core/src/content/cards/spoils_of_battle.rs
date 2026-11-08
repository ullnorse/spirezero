//! `SpoilsOfBattle` (`Models.Cards/SpoilsOfBattle.cs`): Regent common skill, cost 1; Forge 6 (9
//! upgraded), then draw 2.

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct SpoilsOfBattle;

#[sts2_macros::hooks]
impl Hooks for SpoilsOfBattle {
    type Me = CardId;
}

impl CardModel for SpoilsOfBattle {
    fn name(&self) -> &'static str {
        "Spoils of Battle"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Forge, 6).upgrade(3),
            CardVar::new(Var::Cards, 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let forge = card.var(Var::Forge);
        let cards = card.var(Var::Cards);
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        eng.draw(cards, false)?;
        Ok(())
    }
}
