//! `CrescentSpear` (`Models.Cards/CrescentSpear.cs`): Regent common attack, cost 1 and 1 star; deal
//! 8 damage plus 2 (3 upgraded) for each card in the combat piles with a star cost.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct CrescentSpear;

#[sts2_macros::hooks]
impl Hooks for CrescentSpear {
    type Me = CardId;
}

impl CardModel for CrescentSpear {
    fn name(&self) -> &'static str {
        "Crescent Spear"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 8),
            CardVar::new(Var::ExtraDamage, 2).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, star_cost_cards),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("CrescentSpear needs a target");
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

/// `PlayerCombatState.AllCards.Count(c => c.CanonicalStarCost >= 0 || c.HasStarCostX)`.
fn star_cost_cards(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let count = PileType::LISTENER_ORDER
        .into_iter()
        .flat_map(|p| state.player.pile(p).iter().copied())
        .filter(|&c| {
            let kind = state.card(c).kind;
            kind.star_cost() >= 0 || kind.has_star_cost_x()
        })
        .count();
    Decimal::from(i32::try_from(count).expect("a combat has few cards"))
}
