//! `BouncingFlask` (`Models.Cards/BouncingFlask.cs`): cost 2, 3 Poison to a random hittable enemy, 3 times (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct BouncingFlask;

#[sts2_macros::hooks]
impl Hooks for BouncingFlask {
    type Me = CardId;
}

impl CardModel for BouncingFlask {
    fn name(&self) -> &'static str {
        "Bouncing Flask"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::RandomEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Poison), 3),
            CardVar::new(Var::Repeat, 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        let repeat = eng.s.card(play.card).int_var(Var::Repeat);
        let player = eng.s.player.creature;
        for _ in 0..repeat {
            let hittable = eng.hooked().hittable_enemies();
            // `Rng.CombatTargets.NextItem(HittableEnemies)`; null when there is none.
            let Some(enemy) = eng.random_target(&hittable) else {
                continue;
            };
            eng.apply_power_from_card(
                Some(play.card),
                enemy,
                PowerKind::Poison,
                amount,
                Some(player),
            )?;
        }
        Ok(())
    }
}
