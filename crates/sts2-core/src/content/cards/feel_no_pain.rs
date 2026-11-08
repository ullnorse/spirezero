//! `FeelNoPain` (`Models.Cards/FeelNoPain.cs`): apply 3 Feel No Pain (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct FeelNoPain;

#[sts2_macros::hooks]
impl Hooks for FeelNoPain {
    type Me = CardId;
}

impl CardModel for FeelNoPain {
    fn name(&self) -> &'static str {
        "Feel No Pain"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Power"), 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).int_var(Var::Named("Power"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::FeelNoPain,
            Decimal::from(amount),
            Some(player),
        )
    }
}
