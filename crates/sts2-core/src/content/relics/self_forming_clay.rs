//! `SelfFormingClay` (`Models.Relics/SelfFormingClay.cs`): whenever the owner loses HP in combat
//! it gains 3 `SelfFormingClayPower` (Block next turn).

use crate::combat::hooks::{DamageEvent, Hooks};
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DynamicVar("BlockNextTurn", 3m)`.
const BLOCK_NEXT_TURN: Decimal = Decimal::new(3, 0);

pub struct SelfFormingClay;

impl RelicModel for SelfFormingClay {
    fn name(&self) -> &'static str {
        "Self-Forming Clay"
    }
}

#[sts2_macros::hooks]
impl Hooks for SelfFormingClay {
    type Me = RelicId;

    fn after_damage_received(&self, eng: &mut Engine<'_>, _me: RelicId, hit: DamageEvent) -> Flow {
        let player = eng.s.player.creature;
        if eng.s.in_progress && hit.target == player && hit.result.unblocked > 0 {
            eng.apply_power(
                player,
                PowerKind::SelfFormingClay,
                BLOCK_NEXT_TURN,
                Some(player),
            )?;
        }
        Ok(())
    }
}
