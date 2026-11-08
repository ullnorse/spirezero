//! `Stack` (`Models.Cards/Stack.cs`): Event skill, cost 1; Block equal to the number of cards in
//! the discard pile (`0 + 1 ×` count, base 3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType, ValueProps,
};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Stack;

#[sts2_macros::hooks]
impl Hooks for Stack {
    type Me = CardId;
}

impl CardModel for Stack {
    fn name(&self) -> &'static str {
        "Stack"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0).upgrade(3),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::CalculatedBlock, discard_pile_size),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng
            .s
            .calculate(play.card, Var::CalculatedBlock, play.target)
            .expect("Stack has a CalculatedBlock var");
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}

/// `PileType.Discard.GetPile(card.Owner).Cards.Count()`.
fn discard_pile_size(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(
        i32::try_from(s.player.pile(PileType::Discard).len()).expect("a combat has few cards"),
    )
}
