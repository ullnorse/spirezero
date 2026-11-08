//! `RegenPotion` (`Models.Potions/RegenPotion.cs`): the target player gains 5 Regen. It cannot be generated in combat (no in-scope combat generator draws potions).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<RegenPower>(5m)`.
const REGEN: i32 = 5;

pub struct RegenPotion;

#[sts2_macros::hooks]
impl Hooks for RegenPotion {
    type Me = PotionId;
}

impl PotionModel for RegenPotion {
    fn name(&self) -> &'static str {
        "Regen Potion"
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
        eng.apply_power(target, PowerKind::Regen, Decimal::from(REGEN), Some(owner))
    }
}
