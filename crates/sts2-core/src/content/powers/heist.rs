//! `HeistPower` (`Models.Powers/HeistPower.cs`): an `Instanced` buff (`Counter`, the stolen gold) on
//! `FatGremlin`, one per `ThieveryPower` the merc had. When the fat gremlin dies, the gold comes
//! back as an extra reward (`GoldReward(Amount, player, wasGoldStolenBack: true)`); an escape drops
//! the power without a callback. The run history entry (`MarkLootReturned`) is not modelled.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, ExtraReward, Flow};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Heist;

/// The instance's `Target`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub target: Option<CreatureId>,
}

/// `new HeistPower { Target = target }` applied to `owner` with `amount`, by `applier`.
pub fn apply(
    eng: &mut Engine<'_>,
    owner: CreatureId,
    target: CreatureId,
    amount: i32,
    applier: CreatureId,
) -> Flow {
    let instance = eng.apply_power_instance(
        owner,
        PowerKind::Heist,
        Decimal::from(amount),
        Some(applier),
    )?;
    if let Some(me) = instance {
        eng.s.set_power_data(
            me.id,
            PowerData::Heist(Data {
                target: Some(target),
            }),
        );
    }
    Ok(())
}

impl PowerModel for Heist {
    fn name(&self) -> &'static str {
        "Heist"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Heist(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Heist {
    type Me = PowerRef;

    fn before_death(&self, eng: &mut Engine<'_>, me: PowerRef, creature: CreatureId) -> Flow {
        if me.owner == creature {
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.s.extra_rewards.push(ExtraReward::Gold(amount));
        }
        Ok(())
    }
}
