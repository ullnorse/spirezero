//! `TheSmith` (`Models.Cards/TheSmith.cs`): Regent rare skill, cost 1 and 4 stars; Forge 30 (40
//! upgraded).

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct TheSmith;

#[sts2_macros::hooks]
impl Hooks for TheSmith {
    type Me = CardId;
}

impl CardModel for TheSmith {
    fn name(&self) -> &'static str {
        "The Smith"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        4
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Forge, 30).upgrade(10)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let forge = eng.s.card(play.card).var(Var::Forge);
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}
