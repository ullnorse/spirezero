//! `PreciseCut` (`Models.Cards/PreciseCut.cs`): cost 0, 13 damage (16 upgraded) minus 2 per other card in hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct PreciseCut;

#[sts2_macros::hooks]
impl Hooks for PreciseCut {
    type Me = CardId;
}

impl CardModel for PreciseCut {
    fn name(&self) -> &'static str {
        "Precise Cut"
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
            CardVar::new(Var::CalculationBase, 13).upgrade(3),
            CardVar::new(Var::ExtraDamage, 2),
            CardVar::calculated(Var::CalculatedDamage, negative_hand_size),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("PreciseCut needs a target");
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

/// `-(cards in hand)`, not counting the card itself while it is still in the hand.
fn negative_hand_size(s: &CombatState, card: CardId, _target: Option<CreatureId>) -> Decimal {
    let mut count = s.player.hand.len();
    if s.pile_of(card) == Some(PileType::Hand) {
        count -= 1;
    }
    Decimal::ZERO - Decimal::from(i32::try_from(count).expect("the hand is small"))
}
