//! `PhialHolster` (`Models.Relics/PhialHolster.cs`): `AfterObtained` adds 1 potion slot, then
//! procures 2 random out-of-combat potions drawn on the run's `CombatPotionGeneration`.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::potions::random_potions;
use crate::run::{RunCtx, RunError};

/// `DynamicVar("PotionSlots", 1m)`.
const POTION_SLOTS: usize = 1;
/// `DynamicVar("Potions", 2m)`.
const POTIONS: usize = 2;

pub struct PhialHolster;

impl RelicModel for PhialHolster {
    fn name(&self) -> &'static str {
        "Phial Holster"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.gain_max_potion_count(POTION_SLOTS);
        for potion in random_potions(
            s.character,
            POTIONS,
            &mut s.rng[RunRngType::CombatPotionGeneration],
        ) {
            s.procure_potion(potion);
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PhialHolster {
    type Me = RelicId;
}
