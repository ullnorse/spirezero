//! `DeadlyPoison` (`Models.Cards/DeadlyPoison.cs`): cost 1, applies 5 Poison (7 upgraded) to one enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct DeadlyPoison;

#[sts2_macros::hooks]
impl Hooks for DeadlyPoison {
    type Me = CardId;
}

impl CardModel for DeadlyPoison {
    fn name(&self) -> &'static str {
        "Deadly Poison"
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
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Poison), 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("DeadlyPoison needs a target");
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Poison,
            amount,
            Some(player),
        )
    }
}
