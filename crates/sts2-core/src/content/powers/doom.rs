//! `DoomPower` (`Models.Powers/DoomPower.cs`): at the end of its side's turn, a creature whose
//! HP is at most `Amount` dies. The VFX and `Cmd.Wait`s of `DoomKill` have no game effect.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};

pub struct Doom;

impl PowerModel for Doom {
    fn name(&self) -> &'static str {
        "Doom"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

/// `DoomPower.DoomKill`: kills each creature in turn, then runs `Hook.AfterDiedToDoom` once
/// with all of them.
pub fn doom_kill(eng: &mut Engine<'_>, creatures: &[CreatureId]) -> Flow {
    if creatures.is_empty() {
        return Ok(());
    }
    for &creature in creatures {
        eng.kill(&[creature])?;
    }
    eng.after_died_to_doom(creatures)
}

/// `DoomPower.GetDoomedCreatures`: the creatures whose Doom is at least their HP.
pub fn doomed_creatures(s: &CombatState, creatures: &[CreatureId]) -> Vec<CreatureId> {
    creatures
        .iter()
        .copied()
        .filter(|&c| is_doomed(s, c))
        .collect()
}

/// `GetPower<DoomPower>()?.IsOwnerDoomed()`: `Owner.CurrentHp <= Amount`.
fn is_doomed(s: &CombatState, creature: CreatureId) -> bool {
    let creature = s.creature(creature);
    creature.has_power(PowerKind::Doom) && creature.hp <= creature.power_amount(PowerKind::Doom)
}

/// `DoomPower.ShouldDoomTrigger`: only the first doomed creature of the side triggers the kill
/// of all of them.
fn should_doom_trigger(eng: &Engine<'_>, me: PowerRef, participants: &[CreatureId]) -> bool {
    if eng.hooked().is_over_or_ending() {
        return false;
    }
    if !participants.contains(&me.owner) {
        return false;
    }
    let owner = eng.s.creature(me.owner);
    if owner.is_dead() || !is_doomed(eng.s, me.owner) {
        return false;
    }
    let doomed = doomed_creatures(eng.s, &eng.s.creatures_on(owner.side));
    doomed.first() == Some(&me.owner)
}

#[sts2_macros::hooks]
impl Hooks for Doom {
    type Me = PowerRef;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if side != Side::Player && should_doom_trigger(eng, me, participants) {
            let doomed = doomed_creatures(eng.s, &eng.s.creatures_on(side));
            doom_kill(eng, &doomed)?;
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if side != Side::Enemy && should_doom_trigger(eng, me, participants) {
            let doomed = doomed_creatures(eng.s, &eng.s.creatures_on(side));
            doom_kill(eng, &doomed)?;
        }
        Ok(())
    }
}
