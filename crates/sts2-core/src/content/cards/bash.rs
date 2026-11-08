//! `Bash` (`Models.Cards/Bash.cs`): cost 2, 8 damage (10 upgraded) and 2 Vulnerable (3 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Bash;

#[sts2_macros::hooks]
impl Hooks for Bash {
    type Me = CardId;
}

impl CardModel for Bash {
    fn name(&self) -> &'static str {
        "Bash"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 8).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Bash needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable));
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            amount,
            Some(player),
        )
    }
}
