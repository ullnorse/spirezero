//! `ExplosiveAmpoule` (`Models.Potions/ExplosiveAmpoule.cs`): deal 10 unpowered damage to every hittable enemy.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `DamageVar(10m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 10;

pub struct ExplosiveAmpoule;

#[sts2_macros::hooks]
impl Hooks for ExplosiveAmpoule {
    type Me = PotionId;
}

impl PotionModel for ExplosiveAmpoule {
    fn name(&self) -> &'static str {
        "Explosive Ampoule"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target;
        let player = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        // CreatureCmd.Damage(ctx, targets, damage.BaseValue, damage.Props, player, null, null).
        eng.damage(
            &targets,
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(player),
            None,
        )?;
        Ok(())
    }
}
