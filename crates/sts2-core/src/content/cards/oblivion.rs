//! `Oblivion` (`Models.Cards/Oblivion.cs`): cost 0 skill; applies 3 Oblivion (4 upgraded) to the
//! target.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Oblivion;

#[sts2_macros::hooks]
impl Hooks for Oblivion {
    type Me = CardId;
}

impl CardModel for Oblivion {
    fn name(&self) -> &'static str {
        "Oblivion"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Doom), 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Oblivion needs a target");
        let amount = eng.s.card(play.card).int_var(Var::Power(PowerKind::Doom));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Oblivion,
            Decimal::from(amount),
            Some(player),
        )
    }
}
