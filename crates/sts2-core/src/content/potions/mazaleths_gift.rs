//! `MazalethsGift` (`Models.Potions/MazalethsGift.cs`): the target player gains 1 Ritual.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<RitualPower>(1m)`.
const RITUAL: i32 = 1;

pub struct MazalethsGift;

#[sts2_macros::hooks]
impl Hooks for MazalethsGift {
    type Me = PotionId;
}

impl PotionModel for MazalethsGift {
    fn name(&self) -> &'static str {
        "Mazaleth's Gift"
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
            PowerKind::Ritual,
            Decimal::from(RITUAL),
            Some(owner),
        )
    }
}
