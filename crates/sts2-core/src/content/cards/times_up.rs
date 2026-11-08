//! `TimesUp` (`Models.Cards/TimesUp.cs`): cost 2; damage equal to the target's Doom
//! (`0 + 1 × Doom`); upgrading adds Retain.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct TimesUp;

#[sts2_macros::hooks]
impl Hooks for TimesUp {
    type Me = CardId;
}

impl CardModel for TimesUp {
    fn name(&self) -> &'static str {
        "Time's Up"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, target_doom),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("TimesUp needs a target");
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

/// `target?.GetPowerAmount<DoomPower>() ?? 0`.
fn target_doom(state: &CombatState, _card: CardId, target: Option<CreatureId>) -> Decimal {
    Decimal::from(target.map_or(0, |t| state.creature(t).power_amount(PowerKind::Doom)))
}
