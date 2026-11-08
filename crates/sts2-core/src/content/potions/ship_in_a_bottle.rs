//! `ShipInABottle` (`Models.Potions/ShipInABottle.cs`): the target player gains 10 unpowered
//! Block and a `BlockNextTurnPower` of 10.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, ValueProps};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `BlockVar(10m, ValueProp.Unpowered)`.
const BLOCK: i32 = 10;

pub struct ShipInABottle;

#[sts2_macros::hooks]
impl Hooks for ShipInABottle {
    type Me = PotionId;
}

impl PotionModel for ShipInABottle {
    fn name(&self) -> &'static str {
        "Ship in a Bottle"
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
        eng.gain_block(target, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        eng.apply_power(
            target,
            PowerKind::BlockNextTurn,
            Decimal::from(BLOCK),
            Some(owner),
        )
    }
}
