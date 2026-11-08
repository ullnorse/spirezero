//! `ShacklingPotion` (`Models.Potions/ShacklingPotion.cs`): every hittable enemy loses 7
//! Strength until the end of its turn (`ShacklingPotionPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(7m)`.
const STRENGTH: i32 = 7;

pub struct ShacklingPotion;

#[sts2_macros::hooks]
impl Hooks for ShacklingPotion {
    type Me = PotionId;
}

impl PotionModel for ShacklingPotion {
    fn name(&self) -> &'static str {
        "Shackling Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target;
        let owner = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all(
            &targets,
            PowerKind::ShacklingPotion,
            Decimal::from(STRENGTH),
            Some(owner),
        )
    }
}
