//! `DieForYouPower` (`Models.Powers/DieForYouPower.cs`), on Osty: unblocked powered attack
//! damage aimed at Osty's owner goes to Osty while it is alive. Osty is never removed from
//! combat on death, and this power survives it.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

/// Osty's damage redirection.
pub struct DieForYou;

impl PowerModel for DieForYou {
    fn name(&self) -> &'static str {
        "Die for You"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn should_be_removed_after_owner_death(&self) -> bool {
        false
    }
}

#[sts2_macros::hooks]
impl Hooks for DieForYou {
    type Me = PowerRef;

    fn modify_unblocked_damage_target(
        &self,
        s: &CombatState,
        me: PowerRef,
        target: CreatureId,
        _amount: Decimal,
        props: ValueProps,
        _dealer: Option<CreatureId>,
    ) -> CreatureId {
        let osty = s.creature(me.owner);
        if Some(target) != osty.pet_owner || osty.is_dead() || !props.is_powered_attack() {
            return target;
        }
        me.owner
    }

    fn should_creature_be_removed_after_death(
        &self,
        _s: &CombatState,
        me: PowerRef,
        creature: CreatureId,
    ) -> bool {
        creature != me.owner
    }

    fn should_allow_hitting(&self, s: &CombatState, _me: PowerRef, creature: CreatureId) -> bool {
        s.creature(creature).is_alive()
    }
}
