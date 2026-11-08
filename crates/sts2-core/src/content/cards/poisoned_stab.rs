//! `PoisonedStab` (`Models.Cards/PoisonedStab.cs`): cost 1, 6 damage (8 upgraded), then 3 Poison (4 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct PoisonedStab;

#[sts2_macros::hooks]
impl Hooks for PoisonedStab {
    type Me = CardId;
}

impl CardModel for PoisonedStab {
    fn name(&self) -> &'static str {
        "Poisoned Stab"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 6).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Poison), 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("PoisonedStab needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Poison,
            amount,
            Some(player),
        )
    }
}
