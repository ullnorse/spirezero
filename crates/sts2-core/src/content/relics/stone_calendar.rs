//! `StoneCalendar` (`Models.Relics/StoneCalendar.cs`): 52 damage to every hittable enemy at the
//! end of the owner's 7th turn. (`Status` only drives the relic's glow.)

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(52m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 52;
/// `DynamicVar("DamageTurn", 7m)`.
const DAMAGE_TURN: u32 = 7;

pub struct StoneCalendar;

impl RelicModel for StoneCalendar {
    fn name(&self) -> &'static str {
        "Stone Calendar"
    }
}

#[sts2_macros::hooks]
impl Hooks for StoneCalendar {
    type Me = RelicId;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number == DAMAGE_TURN {
            let targets = eng.hooked().hittable_enemies();
            eng.damage(
                &targets,
                Decimal::from(DAMAGE),
                ValueProps::UNPOWERED,
                Some(owner),
                None,
            )?;
        }
        Ok(())
    }
}
