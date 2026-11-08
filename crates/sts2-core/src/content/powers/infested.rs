//! `InfestedPower` (`Models.Powers/InfestedPower.cs`): when its owner dies, four stunned
//! `Wriggler`s join the combat, one per `wriggler<n>` slot of `PhrogParasiteElite`. Keeps
//! combat from ending meanwhile. A buff (`StackType` `Single`). The node visibility and the
//! reveal wait are UI only.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters::{MonsterKind, Wriggler};
use crate::content::powers::{PowerModel, PowerType};

pub struct Infested;

/// `PhrogParasiteElite.GetWrigglerSlotName(0..4)`.
const WRIGGLER_SLOTS: [&str; 4] = ["wriggler1", "wriggler2", "wriggler3", "wriggler4"];

impl PowerModel for Infested {
    fn name(&self) -> &'static str {
        "Infested"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Infested {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if !was_removal_prevented && me.owner == creature {
            for slot in WRIGGLER_SLOTS {
                let wriggler = MonsterKind::Wriggler(Wriggler {
                    start_stunned: true,
                });
                eng.add_enemy(wriggler, Some(slot))?;
            }
        }
        Ok(())
    }

    fn should_stop_combat_from_ending(&self, _s: &CombatState, _me: PowerRef) -> bool {
        true
    }
}
