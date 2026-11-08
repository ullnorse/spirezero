//! `GamePiece` (`Models.Relics/GamePiece.cs`): draw 1 card after the owner plays a Power.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(1)`.
const CARDS: Decimal = Decimal::ONE;

pub struct GamePiece;

impl RelicModel for GamePiece {
    fn name(&self) -> &'static str {
        "Game Piece"
    }
}

#[sts2_macros::hooks]
impl Hooks for GamePiece {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        // The only player owns every card.
        if eng.s.in_progress && eng.s.card(play.card).card_type() == CardType::Power {
            eng.draw(CARDS, false)?;
        }
        Ok(())
    }
}
