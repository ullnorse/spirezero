//! `Bully` (`Models.Cards/Bully.cs`): cost 0, 4 damage (`CalculationBase`) plus 2 (3 upgraded, `ExtraDamage`)
//! per Vulnerable stack on the target.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Bully;

#[sts2_macros::hooks]
impl Hooks for Bully {
    type Me = CardId;
}

impl CardModel for Bully {
    fn name(&self) -> &'static str {
        "Bully"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 4),
            CardVar::new(Var::ExtraDamage, 2).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, vulnerable_amount),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Bully needs a target");
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Calculated(play.card),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// `target?.GetPowerAmount<VulnerablePower>() ?? 0`.
fn vulnerable_amount(state: &CombatState, _card: CardId, target: Option<CreatureId>) -> Decimal {
    Decimal::from(target.map_or(0, |t| state.creature(t).power_amount(PowerKind::Vulnerable)))
}
