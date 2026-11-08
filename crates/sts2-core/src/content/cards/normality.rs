//! `Normality` (`Models.Cards/Normality.cs`): curse, unplayable. While it is in hand, no card
//! can be played once 3 cards were played this turn (`ShouldPlay`).

use crate::combat::AutoPlayType;
use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryTag};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Normality;

/// `_numOfCardsPerTurn`.
const CARDS_PER_TURN: usize = 3;

#[sts2_macros::hooks]
impl Hooks for Normality {
    type Me = CardId;

    /// `ShouldPlay(card, _)`: true unless this card is in hand and 3 plays already started this
    /// turn (`CardPlaysStarted` by the owner; single player, so every card is the owner's).
    fn should_play(
        &self,
        s: &CombatState,
        me: CardId,
        _card: CardId,
        _auto_play: AutoPlayType,
    ) -> bool {
        if s.pile_of(me) != Some(PileType::Hand) {
            return true;
        }
        cards_played_this_turn(s) < CARDS_PER_TURN
    }
}

/// `CardsPlayedThisTurn`: the owner's `CardPlaysStarted` this turn.
fn cards_played_this_turn(s: &CombatState) -> usize {
    let player = s.player.creature;
    s.history_this_turn(HistoryTag::CardPlayStarted)
        .iter()
        .filter(|e| e.actor == player)
        .count()
}

/// `Math.Min(3, CardsPlayedThisTurn)`.
fn capped_cards_played(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let played = cards_played_this_turn(s).min(CARDS_PER_TURN);
    Decimal::from(i32::try_from(played).expect("at most 3"))
}

impl CardModel for Normality {
    fn name(&self) -> &'static str {
        "Normality"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Curse
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Curse
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 3),
            CardVar::new(Var::CalculationExtra, -1),
            CardVar::calculated(Var::Named("CalculatedCards"), capped_cards_played),
        ];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
