//! `Debilitate` (`Models.Cards/Debilitate.cs`): cost 1, 10 damage (12 upgraded), then 2 Debilitate (3 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Debilitate;

#[sts2_macros::hooks]
impl Hooks for Debilitate {
    type Me = CardId;
}

impl CardModel for Debilitate {
    fn name(&self) -> &'static str {
        "Debilitate"
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
            CardVar::new(Var::Damage, 10).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Debilitate), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Debilitate needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let debilitate = eng.s.card(play.card).var(Var::Power(PowerKind::Debilitate));
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Debilitate,
            debilitate,
            Some(player),
        )
    }
}
