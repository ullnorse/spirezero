//! `NecroMasteryPower` (`Models.Powers/NecroMasteryPower.cs`): when the owner's Osty loses HP,
//! deals that much times `Amount` unblockable, unpowered damage to all hittable enemies.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::monsters::Monster;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct NecroMastery;

impl PowerModel for NecroMastery {
    fn name(&self) -> &'static str {
        "Necro Mastery"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for NecroMastery {
    type Me = PowerRef;

    fn after_current_hp_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        delta: Decimal,
    ) -> Flow {
        let osty = eng.s.creature(creature);
        // `creature.PetOwner == Owner.Player`: the owner is the player.
        if delta >= Decimal::ZERO
            || !osty.monster.as_ref().is_some_and(Monster::is_osty)
            || osty.pet_owner != Some(me.owner)
        {
            return Ok(());
        }
        let amount = (Decimal::ZERO - delta) * Decimal::from(eng.s.power(me.owner, me.id).amount);
        let enemies = eng.hooked().hittable_enemies();
        eng.damage(
            &enemies,
            amount,
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            Some(me.owner),
            None,
        )?;
        Ok(())
    }
}
