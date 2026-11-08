//! `Rampage` (`Models.Cards/Rampage.cs`): cost 1; 10 damage, then this card's damage grows by 5
//! (10 upgraded) for the rest of the combat.
//!
//! The game adds the growth to the `DamageVar`'s base value and to `_extraDamageFromPlays`, and
//! `AfterDowngraded` (which resets the vars to the canonical ones) adds the latter back. The card
//! keeps only the sum of the growth, so its damage is `10 + extra` before and after a downgrade.
//! The upgrade changes only `Increase`; the damage itself starts at 10 either way.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardData, CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Rampage;

/// `ExtraDamageFromPlays`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub extra_damage_from_plays: i32,
}

const fn extra_damage(card: &Card) -> i32 {
    match card.data {
        CardData::Rampage(data) => data.extra_damage_from_plays,
        _ => 0,
    }
}

#[sts2_macros::hooks]
impl Hooks for Rampage {
    type Me = CardId;
}

impl CardModel for Rampage {
    fn name(&self) -> &'static str {
        "Rampage"
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
            CardVar::new(Var::Damage, 10),
            CardVar::new(Var::Named("Increase"), 5).upgrade(5),
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
        let target = play.target.expect("Rampage needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let extra = extra_damage(eng.s.card(play.card));
        let increase = eng.s.card(play.card).int_var(Var::Named("Increase"));
        eng.s.card_mut(play.card).data = CardData::Rampage(Data {
            extra_damage_from_plays: extra + increase,
        });
        Ok(())
    }
}
