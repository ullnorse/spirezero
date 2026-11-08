//! `PotionOfDoom` (`Models.Potions/PotionOfDoom.cs`): the target enemy gains 33 Doom.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<DoomPower>(33m)`.
const DOOM: i32 = 33;

pub struct PotionOfDoom;

#[sts2_macros::hooks]
impl Hooks for PotionOfDoom {
    type Me = PotionId;
}

impl PotionModel for PotionOfDoom {
    fn name(&self) -> &'static str {
        "Potion of Doom"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let owner = eng.s.player.creature;
        // PowerCmd.Apply<...>(ctx, target, amount, Owner.Creature, null).
        eng.apply_power(target, PowerKind::Doom, Decimal::from(DOOM), Some(owner))
    }
}
