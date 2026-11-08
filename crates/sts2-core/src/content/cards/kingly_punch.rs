//! `KinglyPunch` (`Models.Cards/KinglyPunch.cs`): Regent uncommon attack, cost 1; deal 8 damage (10
//! upgraded). Each time the card is drawn its damage grows by `Increase` (4, 6 upgraded) for the
//! rest of the combat.
//!
//! The game adds the growth to the `DamageVar`'s base value and to `_extraDamage`, and
//! `AfterDowngraded` (which resets the vars to the canonical ones) adds the latter back. The card
//! keeps only the sum of the growth, so its damage is the canonical one plus `extra_damage`
//! before and after a downgrade (as `Rampage`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardData, CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct KinglyPunch;

/// `_extraDamage`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub extra_damage: i32,
}

const fn extra_damage(card: &Card) -> i32 {
    match card.data {
        CardData::KinglyPunch(data) => data.extra_damage,
        _ => 0,
    }
}

#[sts2_macros::hooks]
impl Hooks for KinglyPunch {
    type Me = CardId;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        if card != me {
            return Ok(());
        }
        let increase = eng.s.card(me).int_var(Var::Named("Increase"));
        let extra = extra_damage(eng.s.card(me));
        eng.s.card_mut(me).data = CardData::KinglyPunch(Data {
            extra_damage: extra + increase,
        });
        Ok(())
    }
}

impl CardModel for KinglyPunch {
    fn name(&self) -> &'static str {
        "Kingly Punch"
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
            CardVar::new(Var::Named("Increase"), 4).upgrade(2),
        ];
        VARS
    }

    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Damage => value + Decimal::from(extra_damage(card)),
            _ => value,
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("KinglyPunch needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
