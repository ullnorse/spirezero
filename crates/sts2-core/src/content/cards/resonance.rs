//! `Resonance` (`Models.Cards/Resonance.cs`): Regent uncommon skill, cost 1 and 2 stars; gain 1 Strength (2 upgraded), every hittable enemy loses 1 Strength.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Resonance;

#[sts2_macros::hooks]
impl Hooks for Resonance {
    type Me = CardId;
}

impl CardModel for Resonance {
    fn name(&self) -> &'static str {
        "Resonance"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Strength), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let strength = eng.s.card(play.card).var(Var::Power(PowerKind::Strength));
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Strength,
            strength,
            Some(player),
        )?;
        for enemy in eng.hooked().hittable_enemies() {
            eng.apply_power_from_card(
                Some(play.card),
                enemy,
                PowerKind::Strength,
                Decimal::from(-1),
                Some(player),
            )?;
        }
        Ok(())
    }
}
