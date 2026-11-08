//! `Ambergris` (`Models.Potions/Ambergris.cs`): the target player heals 50% of their max HP and,
//! in combat, gets an `AmbergrisPower` (one extra turn). Usable any time; the Event pool gives it
//! out (`NeowsSacrifice`), never the combat generator.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;
use crate::run::{RunError, RunState};

/// `DynamicVar("HealPercent", 50m)`.
const HEAL_PERCENT: i32 = 50;

pub struct Ambergris;

/// `(decimal)MaxHp * HealPercent / 100m`.
fn heal_amount(max_hp: i32) -> Decimal {
    Decimal::from(max_hp) * Decimal::from(HEAL_PERCENT) * Decimal::new(1, 2)
}

#[sts2_macros::hooks]
impl Hooks for Ambergris {
    type Me = PotionId;
}

impl PotionModel for Ambergris {
    fn name(&self) -> &'static str {
        "Ambergris"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::AnyTime
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let owner = eng.s.player.creature;
        eng.heal(target, heal_amount(eng.s.creature(target).max_hp))?;
        // `CombatManager.Instance.IsInProgress`.
        if eng.s.in_progress {
            eng.apply_power(target, PowerKind::Ambergris, Decimal::ONE, Some(owner))?;
        }
        Ok(())
    }

    fn on_use_out_of_combat(&self, s: &mut RunState) -> Result<(), RunError> {
        s.heal(heal_amount(s.max_hp));
        Ok(())
    }
}
