//! `SandpitPower` (`Models.Powers/SandpitPower.cs`): an `Instanced` buff (`Counter`) that
//! `TheInsatiable`'s Liquify puts on itself, aimed at a player (`Target`). It counts down at the
//! start of every enemy turn (late); `FranticEscape` raises it. At 0 it is removed, and unless its
//! owner or the target is dead, `AfterRemoved` kills the target's player and Osty with
//! `force: true` (no death prevention). Everything else it does is visual: creature positions
//! (`UpdateCreaturePositions`, `AfterCreatureAddedToCombat`, `AfterOstyRevived`,
//! `BeforeSideTurnEnd`), the music in `AfterPowerAmountChanged`, and `AfterApplied`'s
//! `_initialAmount`/`_initialTargetPosition`, which only feed those positions.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// The countdown to being eaten.
pub struct Sandpit;

/// The instance's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `Target`: the player creature it will eat.
    pub target: Option<CreatureId>,
}

/// `TheInsatiable.LiquifyMove`'s apply for one target: a new `SandpitPower` with `Target` set,
/// applied to `owner` with `amount`.
pub fn apply(eng: &mut Engine<'_>, owner: CreatureId, target: CreatureId, amount: Decimal) -> Flow {
    let instance = eng.apply_power_instance(owner, PowerKind::Sandpit, amount, Some(owner))?;
    if let Some(me) = instance {
        eng.s.set_power_data(
            me.id,
            PowerData::Sandpit(Data {
                target: Some(target),
            }),
        );
    }
    Ok(())
}

impl PowerModel for Sandpit {
    fn name(&self) -> &'static str {
        "Sandpit"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Sandpit(Data::default())
    }

    fn after_removed(&self, eng: &mut Engine<'_>, power: &Power, old_owner: CreatureId) -> Flow {
        let PowerData::Sandpit(Data {
            target: Some(target),
        }) = *eng.s.power_data(power.id)
        else {
            unreachable!("a Sandpit always has a target");
        };
        if eng.s.creature(old_owner).is_dead() || eng.s.creature(target).is_dead() {
            return Ok(());
        }
        // `AllAffectedCreatures`: `Target.Player.Creature`, then `Target.Pets`; only the player
        // and Osty are killed.
        let affected: Vec<CreatureId> = std::iter::once(eng.s.player.creature)
            .chain(eng.s.player.pets.iter().copied())
            .collect();
        for creature in affected {
            let c = eng.s.creature(creature);
            if c.is_player() || Some(creature) == eng.s.osty() {
                eng.kill_forced(&[creature])?;
            }
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Sandpit {
    type Me = PowerRef;

    fn after_side_turn_start_late(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Enemy {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
