//! `RedMask` (`Models.Relics/RedMask.cs`): 1 Weak on every hittable enemy before the owner's
//! first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<WeakPower>(1m)`.
const WEAK: i32 = 1;

pub struct RedMask;

impl RelicModel for RedMask {
    fn name(&self) -> &'static str {
        "Red Mask"
    }
}

#[sts2_macros::hooks]
impl Hooks for RedMask {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number <= 1 {
            let targets = eng.hooked().hittable_enemies();
            eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(WEAK), Some(owner))?;
        }
        Ok(())
    }
}
