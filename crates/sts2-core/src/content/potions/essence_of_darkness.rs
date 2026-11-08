//! `EssenceOfDarkness` (`Models.Potions/EssenceOfDarkness.cs`): the target player channels one Dark
//! orb per orb slot (`OrbQueue.Capacity`, read once).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, OrbKind, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};

pub struct EssenceOfDarkness;

#[sts2_macros::hooks]
impl Hooks for EssenceOfDarkness {
    type Me = PotionId;
}

impl PotionModel for EssenceOfDarkness {
    fn name(&self) -> &'static str {
        "Essence of Darkness"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        let count = eng.s.player.orb_slots;
        for _ in 0..count {
            eng.channel(OrbKind::Dark)?;
        }
        Ok(())
    }
}
