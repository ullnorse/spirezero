//! `ExpectAFight` (`Models.Cards/ExpectAFight.cs`): cost 3; block `15 + 5 ×` (the player's Strength, at least 0), `16 + 8 ×` upgraded.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct ExpectAFight;

#[sts2_macros::hooks]
impl Hooks for ExpectAFight {
    type Me = CardId;
}

impl CardModel for ExpectAFight {
    fn name(&self) -> &'static str {
        "Expect a Fight"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 15).upgrade(1),
            CardVar::new(Var::CalculationExtra, 5).upgrade(3),
            CardVar::calculated(Var::CalculatedBlock, owner_strength),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng
            .s
            .calculate(play.card, Var::CalculatedBlock, play.target)
            .expect("ExpectAFight has a CalculatedBlock var");
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}

/// `Math.Max(0, card.Owner.Creature.GetPowerAmount<StrengthPower>())`.
fn owner_strength(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(
        s.creature(s.player.creature)
            .power_amount(PowerKind::Strength)
            .max(0),
    )
}
