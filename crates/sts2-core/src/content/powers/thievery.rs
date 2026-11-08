//! `ThieveryPower` (`Models.Powers/ThieveryPower.cs`): an `Instanced` buff (`Counter`, the gold
//! taken per `Steal`) on `GremlinMerc`, one per player. `Steal` takes up to `Amount` gold from its
//! `Target` and adds it to the instance's `Gold` var, which `SurprisePower` hands to the fat gremlin.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Thievery;

/// The instance's `Target` and `DynamicVars.Gold` (`GoldVar(0)`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub target: Option<CreatureId>,
    pub gold: i32,
}

/// The data of the `Thievery` instance `me`.
pub fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Thievery(data) => data,
        _ => unreachable!("a Thievery instance holds Thievery data"),
    }
}

/// `new ThieveryPower { Target = target }` applied to `owner` with `amount`.
pub fn apply(eng: &mut Engine<'_>, owner: CreatureId, target: CreatureId, amount: i32) -> Flow {
    let instance = eng.apply_power_instance(
        owner,
        PowerKind::Thievery,
        Decimal::from(amount),
        Some(owner),
    )?;
    if let Some(me) = instance {
        eng.s.set_power_data(
            me.id,
            PowerData::Thievery(Data {
                target: Some(target),
                gold: 0,
            }),
        );
    }
    Ok(())
}

/// `ThieveryPower.Steal`, on every instance `owner` has.
pub fn steal_all(eng: &mut Engine<'_>, owner: CreatureId) {
    let instances: Vec<PowerRef> = eng
        .s
        .creature(owner)
        .powers
        .iter()
        .filter(|p| p.kind == PowerKind::Thievery)
        .map(|p| PowerRef { owner, id: p.id })
        .collect();
    for me in instances {
        let held = data(eng.s, me);
        let Some(target) = held.target else { continue };
        if eng.s.creature(target).is_dead() || eng.s.player.gold <= 0 {
            continue;
        }
        let amount = eng.s.power(owner, me.id).amount.min(eng.s.player.gold);
        eng.lose_gold(Decimal::from(amount));
        eng.s.set_power_data(
            me.id,
            PowerData::Thievery(Data {
                gold: held.gold + amount,
                ..held
            }),
        );
    }
}

impl PowerModel for Thievery {
    fn name(&self) -> &'static str {
        "Thievery"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Thievery(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Thievery {
    type Me = PowerRef;
}
