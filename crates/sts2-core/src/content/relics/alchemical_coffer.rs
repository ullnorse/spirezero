//! `AlchemicalCoffer` (`Models.Relics/AlchemicalCoffer.cs`): `AfterObtained` adds 4 potion slots,
//! then fills each new slot with a random potion (`CreateRandomPotionsOutOfCombat` on the run's
//! `CombatPotionGeneration` stream, then `TryToProcure` at the slot).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::potions::random_potions;
use crate::run::{RunCtx, RunError};

/// `DynamicVar("PotionSlots", 4m)`.
const POTION_SLOTS: usize = 4;

pub struct AlchemicalCoffer;

impl RelicModel for AlchemicalCoffer {
    fn name(&self) -> &'static str {
        "Alchemical Coffer"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let original_slot_count = s.potions.len();
        s.gain_max_potion_count(POTION_SLOTS);
        let potions = random_potions(
            s.character,
            POTION_SLOTS,
            &mut s.rng[RunRngType::CombatPotionGeneration],
        );
        for (i, id) in potions.into_iter().enumerate() {
            s.procure_potion_at(id, original_slot_count + i);
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for AlchemicalCoffer {
    type Me = RelicId;
}
