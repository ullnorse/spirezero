//! `Thrash` (`Models.Cards/Thrash.cs`): rare attack, cost 1; hit twice for 4 (6 upgraded), then
//! exhaust a random Attack in hand and add its base damage (modified by `ModifyDamage` as if it
//! dealt it) to this card's damage for the rest of the combat.
//!
//! As `Rampage`, the game adds the growth to the `DamageVar`'s base value and to `_extraDamage`,
//! and `AfterDowngraded` adds the latter back to the canonical vars; the card keeps only the
//! growth, so its damage is the canonical one plus `extra_damage` before and after a downgrade.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::{DamageQuery, Hooks};
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardData, CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::{Decimal, PackedDecimal};
use crate::rng::RunRngType;

pub struct Thrash;

/// `ExtraDamage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// Packed: it sits in every card's [`CardData`].
    pub extra_damage: PackedDecimal,
}

const fn extra_damage(card: &Card) -> Decimal {
    match card.data {
        CardData::Thrash(data) => data.extra_damage.get(),
        _ => Decimal::ZERO,
    }
}

/// The base damage `Thrash` reads from `card`: its `CalculatedDamage` (calculated with no
/// target), else its `Damage`, else its `OstyDamage`, else 0 (the game logs a warning).
fn base_damage_of(eng: &Engine<'_>, card: CardId) -> Decimal {
    let c = eng.s.card(card);
    eng.s
        .calculate(card, Var::CalculatedDamage, None)
        .or_else(|| c.try_var(Var::Damage))
        .or_else(|| c.try_var(Var::OstyDamage))
        .unwrap_or(Decimal::ZERO)
}

#[sts2_macros::hooks]
impl Hooks for Thrash {
    type Me = CardId;
}

impl CardModel for Thrash {
    fn name(&self) -> &'static str {
        "Thrash"
    }

    fn cost(&self) -> i32 {
        1
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 4).upgrade(2)];
        VARS
    }

    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Damage => value + extra_damage(card),
            _ => value,
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Thrash needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(2),
        )?;
        // NextItem over the hand's Attacks on CombatCardSelection.
        let attacks: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() == CardType::Attack)
            .collect();
        let Some(&card) = eng.s.rng[RunRngType::CombatCardSelection].next_item(&attacks) else {
            return Ok(());
        };
        let base = base_damage_of(eng, card);
        // Hook.ModifyDamage(null target, owner, damage, Move, the exhausted card, null, All).
        let query = DamageQuery {
            target: None,
            dealer: Some(player),
            props: ValueProps::MOVE,
            card: Some(card),
        };
        let (gain, _) = eng.hooked().modify_damage(query, base);
        let extra = extra_damage(eng.s.card(play.card)) + gain;
        eng.s.card_mut(play.card).data = CardData::Thrash(Data {
            extra_damage: extra.into(),
        });
        eng.exhaust(card, false)
    }
}
