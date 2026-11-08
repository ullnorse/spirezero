//! `SoulStorm` (`Models.Cards/SoulStorm.cs`): cost 1; damage `9 + 4 ×` (the `Soul`s in the
//! exhaust pile), 6 per Soul upgraded.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct SoulStorm;

#[sts2_macros::hooks]
impl Hooks for SoulStorm {
    type Me = CardId;
}

impl CardModel for SoulStorm {
    fn name(&self) -> &'static str {
        "Soul Storm"
    }

    fn cost(&self) -> i32 {
        1
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 9),
            CardVar::new(Var::ExtraDamage, 4).upgrade(2),
            CardVar::calculated(Var::CalculatedDamage, souls_in_exhaust_pile),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SoulStorm needs a target");
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

/// `ExhaustPile.Cards.Count(c => c is Soul)`.
fn souls_in_exhaust_pile(
    state: &CombatState,
    _card: CardId,
    _target: Option<CreatureId>,
) -> Decimal {
    let count = state
        .player
        .exhaust
        .iter()
        .filter(|&&c| state.card(c).kind == CardKind::Soul)
        .count();
    Decimal::from(i32::try_from(count).expect("a combat has few cards"))
}
