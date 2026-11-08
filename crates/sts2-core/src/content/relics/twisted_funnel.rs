//! `TwistedFunnel` (`Models.Relics/TwistedFunnel.cs`): before the owner's first turn, applies 4
//! Poison to every hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<PoisonPower>(4m)`.
const POISON: i32 = 4;

pub struct TwistedFunnel;

impl RelicModel for TwistedFunnel {
    fn name(&self) -> &'static str {
        "Twisted Funnel"
    }
}

#[sts2_macros::hooks]
impl Hooks for TwistedFunnel {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if !participants.contains(&owner) || eng.s.player.turn_number > 1 {
            return Ok(());
        }
        // Flash and the smoke puffs: visual only. The hittable list is read once per loop.
        for enemy in eng.hooked().hittable_enemies() {
            eng.apply_power(enemy, PowerKind::Poison, Decimal::from(POISON), Some(owner))?;
        }
        Ok(())
    }
}
