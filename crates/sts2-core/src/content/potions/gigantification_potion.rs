//! `GigantificationPotion` (`Models.Potions/GigantificationPotion.cs`): the target player's next powered Attack deals triple damage.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<GigantificationPower>(1m)`.
const GIGANTIFICATION: i32 = 1;

pub struct GigantificationPotion;

#[sts2_macros::hooks]
impl Hooks for GigantificationPotion {
    type Me = PotionId;
}

impl PotionModel for GigantificationPotion {
    fn name(&self) -> &'static str {
        "Gigantification Potion"
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
            PowerKind::Gigantification,
            Decimal::from(GIGANTIFICATION),
            Some(owner),
        )
    }
}
