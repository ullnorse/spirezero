//! `HauntPower` (`Models.Powers/HauntPower.cs`): playing a `Soul` deals `Amount` unblockable,
//! unpowered damage to a random hittable enemy (`Rng.CombatTargets`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

pub struct Haunt;

impl PowerModel for Haunt {
    fn name(&self) -> &'static str {
        "Haunt"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Haunt {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.card(play.card).kind != CardKind::Soul || eng.s.player.creature != me.owner {
            return Ok(());
        }
        let enemies = eng.hooked().hittable_enemies();
        let Some(target) = eng.s.rng[RunRngType::CombatTargets]
            .next_item(&enemies)
            .copied()
        else {
            return Ok(());
        };
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(
            &[target],
            amount,
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            None,
            None,
        )?;
        Ok(())
    }
}
