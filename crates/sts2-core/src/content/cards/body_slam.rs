//! `BodySlam` (`Models.Cards/BodySlam.cs`): cost 1 (0 upgraded); damage `0 + 1 ×` the player's current block.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct BodySlam;

#[sts2_macros::hooks]
impl Hooks for BodySlam {
    type Me = CardId;
}

impl CardModel for BodySlam {
    fn name(&self) -> &'static str {
        "Body Slam"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, owner_block),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("BodySlam needs a target");
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

/// `card.Owner.Creature.Block`.
fn owner_block(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(state.creature(state.player.creature).block)
}
