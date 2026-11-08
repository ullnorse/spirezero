//! `PossessSpeedPower` (`Models.Powers/PossessSpeedPower.cs`): a buff (`Single`) remembering the Dexterity
//! its owner took from the player (a negative Dexterity change it applied itself); when the owner
//! dies, the player gets it all back.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct PossessSpeed;

/// `Data`: the sum of the stolen amounts (negative), for the one player creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub stolen: Option<Decimal>,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::PossessSpeed(data) => data,
        _ => unreachable!("a PossessSpeed instance holds PossessSpeed data"),
    }
}

impl PowerModel for PossessSpeed {
    fn name(&self) -> &'static str {
        "Possess Speed"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::PossessSpeed(Data { stolen: None })
    }
}

#[sts2_macros::hooks]
impl Hooks for PossessSpeed {
    type Me = PowerRef;

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        if applier != Some(me.owner)
            || !eng.s.creature(power.owner).is_player()
            || eng.s.power(power.owner, power.id).kind != PowerKind::Dexterity
            || amount >= Decimal::ZERO
        {
            return Ok(());
        }
        let stolen = data(eng.s, me).stolen.unwrap_or(Decimal::ZERO) + amount;
        eng.s.set_power_data(
            me.id,
            PowerData::PossessSpeed(Data {
                stolen: Some(stolen),
            }),
        );
        Ok(())
    }

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if was_removal_prevented || creature != me.owner {
            return Ok(());
        }
        if let Some(stolen) = data(eng.s, me).stolen {
            let player = eng.s.player.creature;
            eng.apply_power(player, PowerKind::Dexterity, Decimal::ZERO - stolen, None)?;
        }
        Ok(())
    }
}
