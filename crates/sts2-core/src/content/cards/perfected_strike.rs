//! `PerfectedStrike` (`Models.Cards/PerfectedStrike.cs`): cost 2, Strike; damage `6 + 2 ×` (the Strike cards in every combat pile), 3 per card upgraded.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct PerfectedStrike;

#[sts2_macros::hooks]
impl Hooks for PerfectedStrike {
    type Me = CardId;
}

impl CardModel for PerfectedStrike {
    fn name(&self) -> &'static str {
        "Perfected Strike"
    }

    fn cost(&self) -> i32 {
        2
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

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 6),
            CardVar::new(Var::ExtraDamage, 2).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, strike_cards),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("PerfectedStrike needs a target");
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

/// `PlayerCombatState.AllCards.Count(c => c.Tags.Contains(CardTag.Strike))`.
fn strike_cards(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let count = PileType::LISTENER_ORDER
        .into_iter()
        .flat_map(|p| state.player.pile(p).iter().copied())
        .filter(|&c| state.card(c).kind.is_strike())
        .count();
    Decimal::from(i32::try_from(count).expect("a combat has few cards"))
}
