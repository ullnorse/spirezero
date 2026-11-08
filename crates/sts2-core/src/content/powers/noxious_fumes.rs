//! `NoxiousFumesPower` (`Models.Powers/NoxiousFumesPower.cs`): at the start of the owner's side's
//! turn every hittable enemy gets `Amount` Poison.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct NoxiousFumes;

impl PowerModel for NoxiousFumes {
    fn name(&self) -> &'static str {
        "Noxious Fumes"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for NoxiousFumes {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            let enemies = eng.hooked().hittable_enemies();
            eng.apply_power_to_all(&enemies, PowerKind::Poison, amount, Some(me.owner))?;
        }
        Ok(())
    }
}
