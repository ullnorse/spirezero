//! `Misery` (`Models.Cards/Misery.cs`): cost 0, 7 damage (9 upgraded, gains Retain). Copies
//! every debuff the target had before the hit onto each other hittable enemy.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::{Card, Power};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::{PowerKind, PowerType};
use crate::decimal::Decimal;

pub struct Misery;

#[sts2_macros::hooks]
impl Hooks for Misery {
    type Me = CardId;
}

impl CardModel for Misery {
    fn name(&self) -> &'static str {
        "Misery"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 7).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Misery needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        // `TypeForCurrentAmount == Debuff`, cloned (`ClonePreservingMutability`) with its amount
        // before the hit. An `ITemporaryPower` (`CrushUnder`) adds its amount to its internally
        // applied power's entry, if that is copied too (the first with that id).
        let debuffs: Vec<Power> = eng
            .s
            .creature(target)
            .powers
            .iter()
            .filter(|p| type_for_current_amount(p.kind, p.amount) == PowerType::Debuff)
            .copied()
            .collect();
        let mut amounts: Vec<i32> = debuffs.iter().map(|p| p.amount).collect();
        for (i, debuff) in debuffs.iter().enumerate() {
            if let Some(stat) = debuff.kind.internally_applied_power()
                && let Some(j) = debuffs.iter().position(|p| p.kind == stat)
            {
                amounts[j] += amounts[i];
            }
        }
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        for enemy in eng.hooked().hittable_enemies() {
            if enemy == target {
                continue;
            }
            for (debuff, &amount) in debuffs.iter().zip(&amounts) {
                if amount != 0 {
                    // FindExistingInstanceForStacking then ModifyAmount, else Apply a clone:
                    // `apply_power_clone_from_card` does both (every in-scope power is
                    // `PowerInstanceType.None`).
                    eng.apply_power_clone_from_card(
                        Some(play.card),
                        enemy,
                        debuff,
                        Decimal::from(amount),
                        debuff.applier,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// `PowerModel.GetTypeForAmount`: a negative counter that allows negatives is a debuff, a
/// negative debuff that does not is a buff.
fn type_for_current_amount(kind: PowerKind, amount: i32) -> PowerType {
    if kind.allows_negative() && amount < 0 {
        PowerType::Debuff
    } else if !kind.allows_negative() && kind.power_type() == PowerType::Debuff && amount < 0 {
        PowerType::Buff
    } else {
        kind.power_type()
    }
}
