//! `Dismantle` (`Models.Cards/Dismantle.cs`): cost 1, 8 damage (10 upgraded), hitting twice if the target is Vulnerable.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Dismantle;

#[sts2_macros::hooks]
impl Hooks for Dismantle {
    type Me = CardId;
}

impl CardModel for Dismantle {
    fn name(&self) -> &'static str {
        "Dismantle"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 8).upgrade(2)];
        VARS
    }

    /// `ShouldGlowGoldInternal`: a hittable enemy is Vulnerable.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        s.hittable_enemies()
            .iter()
            .any(|&e| s.creature(e).has_power(PowerKind::Vulnerable))
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Dismantle needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = if eng.s.creature(target).has_power(PowerKind::Vulnerable) {
            2
        } else {
            1
        };
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
