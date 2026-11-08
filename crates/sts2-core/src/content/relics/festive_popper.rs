//! `FestivePopper` (`Models.Relics/FestivePopper.cs`): 9 unpowered damage to every hittable
//! enemy at the start of the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(9m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 9;

pub struct FestivePopper;

impl RelicModel for FestivePopper {
    fn name(&self) -> &'static str {
        "Festive Popper"
    }
}

#[sts2_macros::hooks]
impl Hooks for FestivePopper {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number == 1 {
            let owner = eng.s.player.creature;
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
