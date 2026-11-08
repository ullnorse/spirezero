//! `NoEscape` (`Models.Cards/NoEscape.cs`): cost 1; Doom equal to 10 (15 upgraded) plus 5 per 10
//! Doom the target already has.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct NoEscape;

#[sts2_macros::hooks]
impl Hooks for NoEscape {
    type Me = CardId;
}

impl CardModel for NoEscape {
    fn name(&self) -> &'static str {
        "No Escape"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Named("DoomThreshold"), 10),
            CardVar::new(Var::CalculationBase, 10).upgrade(5),
            CardVar::new(Var::CalculationExtra, 5),
            CardVar::calculated(Var::Named("CalculatedDoom"), doom_thresholds_reached),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("NoEscape needs a target");
        let amount = eng
            .s
            .calculate(play.card, Var::Named("CalculatedDoom"), Some(target))
            .expect("NoEscape has a CalculatedDoom var");
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Doom,
            amount,
            Some(player),
        )
    }
}

/// `Math.Floor((target?.GetPowerAmount<DoomPower>() ?? 0) / DoomThreshold)`.
fn doom_thresholds_reached(s: &CombatState, card: CardId, target: Option<CreatureId>) -> Decimal {
    let doom = target.map_or(0, |t| s.creature(t).power_amount(PowerKind::Doom));
    let threshold = s.card(card).int_var(Var::Named("DoomThreshold"));
    Decimal::from(doom.div_euclid(threshold))
}
