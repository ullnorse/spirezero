//! `Strangle` (`Models.Cards/Strangle.cs`): cost 1, 8 damage (10 upgraded) then 2 Strangle (1 more upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Strangle;

#[sts2_macros::hooks]
impl Hooks for Strangle {
    type Me = CardId;
}

impl CardModel for Strangle {
    fn name(&self) -> &'static str {
        "Strangle"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 8).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Strangle), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Strangle needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Strangle));
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Strangle,
            amount,
            Some(player),
        )
    }
}
