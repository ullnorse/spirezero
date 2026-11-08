//! `PossessStrengthPower` (`Models.Powers/PossessStrengthPower.cs`): a buff (`Single`) remembering the Strength
//! its owner took from the player (a negative Strength change it applied itself); when the owner
//! dies, the player gets it all back.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct PossessStrength;

/// `Data`: the sum of the stolen amounts (negative), for the one player creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub stolen: Option<Decimal>,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::PossessStrength(data) => data,
        _ => unreachable!("a PossessStrength instance holds PossessStrength data"),
    }
}

impl PowerModel for PossessStrength {
    fn name(&self) -> &'static str {
        "Possess Strength"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::PossessStrength(Data { stolen: None })
    }
}

#[sts2_macros::hooks]
impl Hooks for PossessStrength {
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
            || eng.s.power(power.owner, power.id).kind != PowerKind::Strength
            || amount >= Decimal::ZERO
        {
            return Ok(());
        }
        let stolen = data(eng.s, me).stolen.unwrap_or(Decimal::ZERO) + amount;
        eng.s.set_power_data(
            me.id,
            PowerData::PossessStrength(Data {
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
            eng.apply_power(player, PowerKind::Strength, Decimal::ZERO - stolen, None)?;
        }
        Ok(())
    }
}
