//! `Restlessness` (`Models.Cards/Restlessness.cs`): cost 0, Retain; if it is the only card in the
//! hand, draw 2 cards (3 upgraded) and gain 2 Energy (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Restlessness;

#[sts2_macros::hooks]
impl Hooks for Restlessness {
    type Me = CardId;
}

impl CardModel for Restlessness {
    fn name(&self) -> &'static str {
        "Restlessness"
    }

    fn cost(&self) -> i32 {
        0
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Retain)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Cards, 2).upgrade(1),
            CardVar::new(Var::Energy, 2).upgrade(1),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `IsOnlyCardInHand`.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        is_only_card_in_hand(s, me)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        if !is_only_card_in_hand(eng.s, play.card) {
            return Ok(());
        }
        for _ in 0..eng.s.card(play.card).int_var(Var::Cards) {
            eng.draw(Decimal::ONE, false)?;
        }
        eng.gain_energy(Decimal::from(eng.s.card(play.card).int_var(Var::Energy)))
    }
}

/// `IsOnlyCardInHand`: the hand without this card is empty.
fn is_only_card_in_hand(s: &CombatState, me: CardId) -> bool {
    s.player.hand.iter().all(|&c| c == me)
}
