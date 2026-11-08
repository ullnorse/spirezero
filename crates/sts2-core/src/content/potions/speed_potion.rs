//! `SpeedPotion` (`Models.Potions/SpeedPotion.cs`): the target player gains 5 Dexterity until the end of the turn (`SpeedPotionPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<DexterityPower>(5m)`.
const DEXTERITY: i32 = 5;

pub struct SpeedPotion;

#[sts2_macros::hooks]
impl Hooks for SpeedPotion {
    type Me = PotionId;
}

impl PotionModel for SpeedPotion {
    fn name(&self) -> &'static str {
        "Speed Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let owner = eng.s.player.creature;
        // PowerCmd.Apply<...>(ctx, target, amount, Owner.Creature, null).
        eng.apply_power(
            target,
            PowerKind::SpeedPotion,
            Decimal::from(DEXTERITY),
            Some(owner),
        )
    }
}
