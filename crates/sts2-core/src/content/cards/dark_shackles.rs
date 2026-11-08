//! `DarkShackles` (`Models.Cards/DarkShackles.cs`): cost 0, Exhaust; the target loses 9 (15
//! upgraded) Strength until the end of its turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct DarkShackles;

#[sts2_macros::hooks]
impl Hooks for DarkShackles {
    type Me = CardId;
}

impl CardModel for DarkShackles {
    fn name(&self) -> &'static str {
        "Dark Shackles"
    }

    fn cost(&self) -> i32 {
        0
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
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("StrengthLoss"), 9).upgrade(6)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("DarkShackles needs a target");
        let amount = eng.s.card(play.card).var(Var::Named("StrengthLoss"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::DarkShackles,
            amount,
            Some(player),
        )
    }
}
