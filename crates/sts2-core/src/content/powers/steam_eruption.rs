//! `SteamEruptionPower` (`Models.Powers/SteamEruptionPower.cs`): a buff (`Counter`) on
//! `WaterfallGiant` that outlives its owner's death. When the owner dies it is not removed from
//! combat, and `WaterfallGiant.TriggerAboutToBlowState` brings it back. Until the power goes, combat
//! cannot end.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters::WaterfallGiant;
use crate::content::powers::{PowerModel, PowerType};

pub struct SteamEruption;

impl PowerModel for SteamEruption {
    fn name(&self) -> &'static str {
        "Steam Eruption"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn should_be_removed_after_owner_death(&self) -> bool {
        false
    }
}

#[sts2_macros::hooks]
impl Hooks for SteamEruption {
    type Me = PowerRef;

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
        WaterfallGiant::trigger_about_to_blow_state(eng, me.owner)
    }

    fn should_stop_combat_from_ending(&self, _s: &CombatState, _me: PowerRef) -> bool {
        true
    }

    fn should_creature_be_removed_after_death(
        &self,
        _s: &CombatState,
        me: PowerRef,
        creature: CreatureId,
    ) -> bool {
        creature != me.owner
    }
}
