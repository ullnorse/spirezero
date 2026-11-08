//! `MercuryHourglass` (`Models.Relics/MercuryHourglass.cs`): 3 damage to every hittable enemy at
//! the start of each of the owner's turns.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(3m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 3;

pub struct MercuryHourglass;

impl RelicModel for MercuryHourglass {
    fn name(&self) -> &'static str {
        "Mercury Hourglass"
    }
}

#[sts2_macros::hooks]
impl Hooks for MercuryHourglass {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        eng.damage(
            &targets,
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(owner),
            None,
        )?;
        Ok(())
    }
}
