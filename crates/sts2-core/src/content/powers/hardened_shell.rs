//! `HardenedShellPower` (`Models.Powers/HardenedShellPower.cs`): caps the HP the owner loses per
//! turn at `Amount`: each hit loses at most `Amount` minus what was lost since the last
//! `BeforeSideTurnStart`, which resets the count (for either side). Reaching `Amount` makes the
//! owner's HP display infinite until then (read by `HellraiserPower`).

use crate::combat::hooks::{DamageEvent, Hooks, HpLossQuery, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct HardenedShell;

/// `HardenedShellPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `damageReceivedThisTurn` (a `decimal` that only ever adds `UnblockedDamage` ints).
    pub damage_received_this_turn: i32,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::HardenedShell(data) => data,
        _ => unreachable!("a HardenedShell instance holds HardenedShell data"),
    }
}

impl PowerModel for HardenedShell {
    fn name(&self) -> &'static str {
        "Hardened Shell"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::HardenedShell(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for HardenedShell {
    type Me = PowerRef;

    fn modify_hp_lost_before_osty_late(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner || amount == Decimal::ZERO {
            return amount;
        }
        let left = s.power(me.owner, me.id).amount - data(s, me).damage_received_this_turn;
        amount.min(Decimal::from(left))
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || hit.result.was_fully_blocked {
            return Ok(());
        }
        let damage_received_this_turn =
            data(eng.s, me).damage_received_this_turn + hit.result.unblocked;
        eng.s.set_power_data(
            me.id,
            PowerData::HardenedShell(Data {
                damage_received_this_turn,
            }),
        );
        // `HpDisplay.InfiniteWithNumbers` (`HardenedShellPower.cs:63-67`).
        if damage_received_this_turn >= eng.s.power(me.owner, me.id).amount {
            eng.s.creature_mut(me.owner).infinite_hp_display = true;
        }
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        eng.s
            .set_power_data(me.id, PowerData::HardenedShell(Data::default()));
        eng.s.creature_mut(me.owner).infinite_hp_display = false;
        Ok(())
    }
}
