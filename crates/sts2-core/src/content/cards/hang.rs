//! `Hang` (`Models.Cards/Hang.cs`): cost 1, 10 damage (13 upgraded), then applies `HangPower`
//! (at least 2, or the target's current amount again): later Hang attacks on it do that many
//! times their damage.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Hang;

/// The cap `PowerCmd` applies to stacks.
const MAX_AMOUNT: i32 = 999_999_999;

#[sts2_macros::hooks]
impl Hooks for Hang {
    type Me = CardId;
}

impl CardModel for Hang {
    fn name(&self) -> &'static str {
        "Hang"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Hang needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let power_amount = eng
            .s
            .creature(target)
            .powers
            .iter()
            .find(|p| p.kind == PowerKind::Hang)
            .map_or(0, |p| p.amount);
        let mut amount = power_amount.max(2);
        if power_amount + amount > MAX_AMOUNT {
            amount = (MAX_AMOUNT - power_amount).max(0);
        }
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Hang,
            Decimal::from(amount),
            Some(player),
        )
    }
}
