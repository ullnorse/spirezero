//! `InfusedCore` (`Models.Relics/InfusedCore.cs`): Defect's upgraded starter relic
//! (`TouchOfOrobas`); channels 3 Lightning orbs at the start of the owner's first turn, and the
//! owner's Lightning orbs have 1 more value.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, Orb, OrbKind, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DynamicVar("Lightning", 3m)`.
const LIGHTNING: u32 = 3;
/// `DynamicVar("ExtraDamage", 1m)`.
const EXTRA_DAMAGE: Decimal = Decimal::ONE;

pub struct InfusedCore;

impl RelicModel for InfusedCore {
    fn name(&self) -> &'static str {
        "Infused Core"
    }
}

#[sts2_macros::hooks]
impl Hooks for InfusedCore {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number <= 1 {
            for _ in 0..LIGHTNING {
                eng.channel(OrbKind::Lightning)?;
            }
        }
        Ok(())
    }

    fn modify_orb_value(
        &self,
        _s: &CombatState,
        _me: RelicId,
        orb: Orb,
        value: Decimal,
    ) -> Decimal {
        // Every orb belongs to the only player.
        if orb.kind != OrbKind::Lightning {
            return value;
        }
        value + EXTRA_DAMAGE
    }
}
