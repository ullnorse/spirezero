//! `StableSerum` (`Models.Potions/StableSerum.cs`): the target player keeps their hand for 2 turns (`RetainHandPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `RepeatVar(2)`.
const REPEAT: i32 = 2;

pub struct StableSerum;

#[sts2_macros::hooks]
impl Hooks for StableSerum {
    type Me = PotionId;
}

impl PotionModel for StableSerum {
    fn name(&self) -> &'static str {
        "Stable Serum"
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
            PowerKind::RetainHand,
            Decimal::from(REPEAT),
            Some(owner),
        )
    }
}
