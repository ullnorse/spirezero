//! `GoForTheEyes` (`Models.Cards/GoForTheEyes.cs`): cost 0, 3 damage (4 upgraded); 1 Weak (2 upgraded) if the target
//! intends to attack (`MonsterModel.IntendsToAttack`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct GoForTheEyes;

#[sts2_macros::hooks]
impl Hooks for GoForTheEyes {
    type Me = CardId;
}

impl CardModel for GoForTheEyes {
    fn name(&self) -> &'static str {
        "Go for the Eyes"
    }

    fn cost(&self) -> i32 {
        0
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
            CardVar::new(Var::Damage, 3).upgrade(1),
            CardVar::new(Var::Power(PowerKind::Weak), 1).upgrade(1),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: a hittable enemy intends to attack.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        s.hittable_enemies().iter().any(|&e| s.intends_to_attack(e))
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("GoForTheEyes needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if eng.s.intends_to_attack(target) {
            let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Weak));
            eng.apply_power_from_card(
                Some(play.card),
                target,
                PowerKind::Weak,
                amount,
                Some(player),
            )
        } else {
            Ok(())
        }
    }
}
