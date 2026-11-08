//! `SlowPower` (`Models.Powers/SlowPower.cs`): a debuff whose owner takes 10% more powered attack
//! damage for every card played since the owner's side last started its turn. The count
//! (`DynamicVars["SlowAmount"]`) is reset when the owner's side's turn starts. `DisplayAmount` and
//! `Flash` are UI only.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Slow;

/// `0.1m`, the damage added per card played.
const DAMAGE_PER_CARD: Decimal = Decimal::new(1, 1);

/// `SlowPower.DynamicVars`: the instance's `SlowAmount` (`DisplayAmount` is derived from it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub slow_amount: i32,
}

fn slow_amount(s: &CombatState, me: PowerRef) -> i32 {
    match *s.power_data(me.id) {
        PowerData::Slow(data) => data.slow_amount,
        _ => unreachable!("a Slow instance holds Slow data"),
    }
}

fn set_slow_amount(s: &mut CombatState, me: PowerRef, value: i32) {
    s.set_power_data(me.id, PowerData::Slow(Data { slow_amount: value }));
}

impl PowerModel for Slow {
    fn name(&self) -> &'static str {
        "Slow"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Slow(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Slow {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        let value = slow_amount(eng.s, me) + 1;
        set_slow_amount(eng.s, me, value);
        Ok(())
    }

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        Decimal::ONE + DAMAGE_PER_CARD * Decimal::from(slow_amount(s, me))
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            set_slow_amount(eng.s, me, 0);
        }
        Ok(())
    }
}
