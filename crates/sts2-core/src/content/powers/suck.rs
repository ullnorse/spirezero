//! `SuckPower` (`Models.Powers/SuckPower.cs`): after the owner's powered attack on the other
//! side, it gains `Amount` Strength for each hit that took HP from a target. Hits that Osty (a
//! pet) took HP from count; the pet owner's own result in the same hit is not looked at.

use crate::combat::commands::Attack;
use crate::combat::commands::damage::DamageResult;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Suck;

impl PowerModel for Suck {
    fn name(&self) -> &'static str {
        "Suck"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

/// The number of hits (`command.Results` entries) in which a result took HP, once the pet owner's
/// result is dropped from every hit with a pet. `results` is flat: a hit that Osty's redirect
/// split is the pet's result followed by the original target's.
fn hits_that_took_hp(s: &CombatState, results: &[DamageResult]) -> i32 {
    let mut count = 0;
    let mut i = 0;
    while i < results.len() {
        let r = results[i];
        if s.creature(r.receiver).pet_owner.is_some() {
            // `item.RemoveAll(r => r.Receiver == petHit.Receiver.PetOwner?.Creature)`.
            let owner = s.creature(r.receiver).pet_owner;
            let rest = results.get(i + 1).filter(|n| Some(n.receiver) == owner);
            count += i32::from(r.unblocked > 0);
            i += 1 + usize::from(rest.is_some());
        } else {
            count += i32::from(r.unblocked > 0);
            i += 1;
        }
    }
    count
}

#[sts2_macros::hooks]
impl Hooks for Suck {
    type Me = PowerRef;

    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        attack: &Attack,
        results: &[DamageResult],
    ) -> Flow {
        // `command.TargetSide == Owner.Side` never holds for a monster's attack: it targets the
        // `PlayerCreatures`.
        if attack.attacker != me.owner || !attack.props.is_powered_attack() {
            return Ok(());
        }
        let hits = hits_that_took_hp(eng.s, results);
        if hits > 0 {
            let amount = eng.s.power(me.owner, me.id).amount * hits;
            eng.apply_power(
                me.owner,
                PowerKind::Strength,
                Decimal::from(amount),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
