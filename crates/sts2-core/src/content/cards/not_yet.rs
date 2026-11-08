//! `NotYet` (`Models.Cards/NotYet.cs`): cost 2, Exhaust, heals 10 HP (13 upgraded). `CanBeGeneratedInCombat` is false.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct NotYet;

#[sts2_macros::hooks]
impl Hooks for NotYet {
    type Me = CardId;
}

impl CardModel for NotYet {
    fn name(&self) -> &'static str {
        "Not Yet"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Heal, 10).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.heal(player, eng.s.card(play.card).var(Var::Heal))
    }
}
