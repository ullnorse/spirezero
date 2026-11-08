//! `DelicateFrond` (`Models.Relics/DelicateFrond.cs`): Ancient (Nonupeipe); `BeforeCombatStart`
//! fills every open potion slot with a random potion (`CreateRandomPotionOutOfCombat` on
//! `CombatPotionGeneration`, one call per potion, then `TryToProcure`); it stops at the first
//! refusal (`Sozu`), after that potion was drawn.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::potions::{combat_potion, random_potions};

pub struct DelicateFrond;

impl RelicModel for DelicateFrond {
    fn name(&self) -> &'static str {
        "Delicate Frond"
    }
}

#[sts2_macros::hooks]
impl Hooks for DelicateFrond {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        while let Some(slot) = eng.s.player.first_empty_potion_slot() {
            let id = random_potions(
                eng.s.player.character,
                1,
                &mut eng.s.rng[RunRngType::CombatPotionGeneration],
            )[0];
            if let Some(kind) = combat_potion(id) {
                if !eng.try_to_procure_potion(kind)? {
                    break;
                }
            } else {
                // A potion that is not ported still holds its slot.
                if !eng.hooked().should_procure_potion() {
                    break;
                }
                eng.s.player.unported_potions[slot] = Some(id);
            }
        }
        Ok(())
    }
}
