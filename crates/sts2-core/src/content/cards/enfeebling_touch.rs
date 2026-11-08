//! `EnfeeblingTouch` (`Models.Cards/EnfeeblingTouch.cs`): cost 1, Ethereal; the target loses 8 Strength (11 upgraded) this turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct EnfeeblingTouch;

#[sts2_macros::hooks]
impl Hooks for EnfeeblingTouch {
    type Me = CardId;
}

impl CardModel for EnfeeblingTouch {
    fn name(&self) -> &'static str {
        "Enfeebling Touch"
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
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("StrengthLoss"), 8).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("EnfeeblingTouch needs a target");
        let amount = eng.s.card(play.card).var(Var::Named("StrengthLoss"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::EnfeeblingTouch,
            amount,
            Some(player),
        )
    }
}
