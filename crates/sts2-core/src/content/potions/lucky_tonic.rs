//! `LuckyTonic` (`Models.Potions/LuckyTonic.cs`): the target player gains 1 Buffer.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<BufferPower>(1m)`.
const BUFFER: i32 = 1;

pub struct LuckyTonic;

#[sts2_macros::hooks]
impl Hooks for LuckyTonic {
    type Me = PotionId;
}

impl PotionModel for LuckyTonic {
    fn name(&self) -> &'static str {
        "Lucky Tonic"
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
            PowerKind::Buffer,
            Decimal::from(BUFFER),
            Some(owner),
        )
    }
}
