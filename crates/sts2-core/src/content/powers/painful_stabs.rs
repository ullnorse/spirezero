//! `PainfulStabsPower` (`Models.Powers/PainfulStabsPower.cs`): a buff (`Counter`) on `TestSubject`'s
//! second form. After each of the owner's powered attacks on the other side that took HP from
//! anyone, each player creature it hit gets `Amount` `Wound`s per hit that took its HP, into its
//! discard pile. It survives its owner's death, which keeps the owner in combat.

use crate::combat::PileType;
use crate::combat::commands::Attack;
use crate::combat::commands::damage::DamageResult;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};

pub struct PainfulStabs;

impl PowerModel for PainfulStabs {
    fn name(&self) -> &'static str {
        "Painful Stabs"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn should_be_removed_after_owner_death(&self) -> bool {
        false
    }
}

#[sts2_macros::hooks]
impl Hooks for PainfulStabs {
    type Me = PowerRef;

    fn should_creature_be_removed_after_death(
        &self,
        _s: &CombatState,
        me: PowerRef,
        creature: CreatureId,
    ) -> bool {
        creature != me.owner
    }

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
        if !results.iter().any(|r| r.unblocked > 0) {
            return Ok(());
        }
        // The results grouped by player receiver, in order of first appearance.
        let mut receivers: Vec<(CreatureId, u32)> = Vec::new();
        for r in results {
            if !eng.s.creature(r.receiver).is_player() {
                continue;
            }
            let took_hp = u32::from(r.unblocked > 0);
            match receivers.iter_mut().find(|(c, _)| *c == r.receiver) {
                Some((_, n)) => *n += took_hp,
                None => receivers.push((r.receiver, took_hp)),
            }
        }
        let amount = u32::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        for (receiver, hits) in receivers {
            eng.add_to_combat_and_preview(
                receiver,
                CardKind::Wound,
                PileType::Discard,
                amount * hits,
                None,
            )?;
        }
        Ok(())
    }
}
