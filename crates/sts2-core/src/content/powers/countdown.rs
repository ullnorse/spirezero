//! `CountdownPower` (`Models.Powers/CountdownPower.cs`): at the start of its owner's side's turn,
//! applies `Amount` Doom to a random hittable enemy (`Rng.CombatTargets`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

pub struct Countdown;

impl PowerModel for Countdown {
    fn name(&self) -> &'static str {
        "Countdown"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Countdown {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        // Flash() is presentation only.
        let enemies = eng.hooked().hittable_enemies();
        let target = eng.s.rng[RunRngType::CombatTargets]
            .next_item(&enemies)
            .copied();
        if let Some(target) = target {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.apply_power(target, PowerKind::Doom, amount, Some(me.owner))?;
        }
        Ok(())
    }
}
