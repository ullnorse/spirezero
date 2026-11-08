//! `PetrifiedToad` (`Models.Relics/PetrifiedToad.cs`): a Potion-Shaped Rock at the start of
//! every fight, if a belt slot is free.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::potions::PotionKind;
use crate::content::relics::RelicModel;

pub struct PetrifiedToad;

impl RelicModel for PetrifiedToad {
    fn name(&self) -> &'static str {
        "Petrified Toad"
    }
}

#[sts2_macros::hooks]
impl Hooks for PetrifiedToad {
    type Me = RelicId;

    fn before_combat_start_late(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        eng.try_to_procure_potion(PotionKind::PotionShapedRock)?;
        Ok(())
    }
}
