//! `ScreamingFlagon` (`Models.Relics/ScreamingFlagon.cs`): 20 damage to every hittable enemy when
//! the owner's turn ends with an empty hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(20m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 20;

pub struct ScreamingFlagon;

impl RelicModel for ScreamingFlagon {
    fn name(&self) -> &'static str {
        "Screaming Flagon"
    }
}

#[sts2_macros::hooks]
impl Hooks for ScreamingFlagon {
    type Me = RelicId;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.hand.is_empty() {
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
