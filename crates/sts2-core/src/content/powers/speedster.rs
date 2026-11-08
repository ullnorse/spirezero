//! `SpeedsterPower` (`Models.Powers/SpeedsterPower.cs`): a buff (`Counter`); whenever the owner
//! draws a card outside the hand draw on its own side's turn, `Amount` unpowered damage to every
//! hittable enemy.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Speedster;

impl PowerModel for Speedster {
    fn name(&self) -> &'static str {
        "Speedster"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Speedster {
    type Me = PowerRef;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner.Creature == Owner`: every card belongs to the only player.
        if from_hand_draw
            || eng.s.player.creature != me.owner
            || eng.s.side != eng.s.creature(me.owner).side
        {
            return Ok(());
        }
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
}
