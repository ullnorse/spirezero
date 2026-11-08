//! `OneTwoPunch` (`Models.Cards/OneTwoPunch.cs`): cost 1; apply `OneTwoPunchPower`, so this turn
//! the next Attack (2 upgraded) is played an extra time.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct OneTwoPunch;

#[sts2_macros::hooks]
impl Hooks for OneTwoPunch {
    type Me = CardId;
}

impl CardModel for OneTwoPunch {
    fn name(&self) -> &'static str {
        "One-Two Punch"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Attacks"), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let attacks = eng.s.card(play.card).int_var(Var::Named("Attacks"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::OneTwoPunch,
            Decimal::from(attacks),
            Some(player),
        )
    }
}
