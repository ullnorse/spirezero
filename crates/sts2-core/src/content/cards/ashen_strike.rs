//! `AshenStrike` (`Models.Cards/AshenStrike.cs`): cost 1, Strike; damage `6 + 3 ×` (the exhaust pile's size), 4 per card upgraded.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct AshenStrike;

#[sts2_macros::hooks]
impl Hooks for AshenStrike {
    type Me = CardId;
}

impl CardModel for AshenStrike {
    fn name(&self) -> &'static str {
        "Ashen Strike"
    }

    fn cost(&self) -> i32 {
        1
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

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 6),
            CardVar::new(Var::ExtraDamage, 3).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, exhaust_pile_size),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("AshenStrike needs a target");
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

/// `PileType.Exhaust.GetPile(card.Owner).Cards.Count`.
fn exhaust_pile_size(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i32::try_from(state.player.exhaust.len()).expect("a combat has few cards"))
}
