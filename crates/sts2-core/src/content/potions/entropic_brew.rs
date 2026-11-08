//! `EntropicBrew` (`Models.Potions/EntropicBrew.cs`): fills every open potion slot with a random
//! potion (`PotionFactory.CreateRandomPotionOutOfCombat` on `CombatPotionGeneration`, one call per
//! potion). Usable any time.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::rng::RunRngType;
use crate::run::potions::{combat_potion, random_potions};
use crate::run::{RunError, RunState};

pub struct EntropicBrew;

#[sts2_macros::hooks]
impl Hooks for EntropicBrew {
    type Me = PotionId;
}

impl PotionModel for EntropicBrew {
    fn may_prompt(&self) -> bool {
        // It can brew a potion that prompts (Attack Potion, ...).
        true
    }

    fn name(&self) -> &'static str {
        "Entropic Brew"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::AnyTime
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // while (HasOpenPotionSlots) { if (!TryToProcure(CreateRandomPotionOutOfCombat).success) break; }.
        while let Some(slot) = eng.s.player.first_empty_potion_slot() {
            let id = random_potions(
                eng.s.player.character,
                1,
                &mut eng.s.rng[RunRngType::CombatPotionGeneration],
            )[0];
            match combat_potion(id) {
                Some(kind) => {
                    if !eng.try_to_procure_potion(kind)? {
                        break;
                    }
                }
                // A potion that is not ported still holds its slot (unless `Sozu` vetoes it).
                None if eng.hooked().should_procure_potion() => {
                    eng.s.player.unported_potions[slot] = Some(id);
                }
                None => break,
            }
        }
        Ok(())
    }

    fn on_use_out_of_combat(&self, s: &mut RunState) -> Result<(), RunError> {
        while s.potions.iter().any(Option::is_none) {
            let id = random_potions(
                s.character,
                1,
                &mut s.rng[RunRngType::CombatPotionGeneration],
            )[0];
            if !s.procure_potion(id) {
                break;
            }
        }
        Ok(())
    }
}
