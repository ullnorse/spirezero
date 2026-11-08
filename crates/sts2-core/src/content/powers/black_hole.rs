//! `BlackHolePower` (`Models.Powers/BlackHolePower.cs`): a buff (`Counter`); deals `Amount`
//! unpowered damage to every hittable enemy after the last play of a card that spent stars
//! (stars are spent at the start of a play, so it reacts in `AfterCardPlayed`), and on each
//! positive star gain.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct BlackHole;

impl PowerModel for BlackHole {
    fn name(&self) -> &'static str {
        "Black Hole"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

/// `DealDamageToAllEnemies`.
fn deal_damage_to_all_enemies(eng: &mut Engine<'_>, me: PowerRef) -> Flow {
    let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
    let targets = eng.hooked().hittable_enemies();
    eng.damage(
        &targets,
        amount,
        ValueProps::UNPOWERED,
        Some(me.owner),
        None,
    )?;
    Ok(())
}

#[sts2_macros::hooks]
impl Hooks for BlackHole {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner.Player`: every card belongs to the player.
        if play.stars_spent() > 0 && play.is_last_in_series() {
            deal_damage_to_all_enemies(eng, me)?;
        }
        Ok(())
    }

    fn after_stars_gained(&self, eng: &mut Engine<'_>, me: PowerRef, amount: i32) -> Flow {
        if amount > 0 {
            deal_damage_to_all_enemies(eng, me)?;
        }
        Ok(())
    }
}
