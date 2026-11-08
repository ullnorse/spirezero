//! `RadiantPearl` (`Models.Relics/RadiantPearl.cs`): before the first hand draw of a combat, 1
//! `Luminesce` joins the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, RelicId};
use crate::content::cards::CardKind;
use crate::content::relics::RelicModel;

/// `CardsVar(1)`.
const CARDS: usize = 1;

pub struct RadiantPearl;

impl RelicModel for RadiantPearl {
    fn name(&self) -> &'static str {
        "Radiant Pearl"
    }
}

#[sts2_macros::hooks]
impl Hooks for RadiantPearl {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // The only player is the owner.
        if eng.s.player.turn_number != 1 {
            return Ok(());
        }
        let cards: Vec<_> = (0..CARDS)
            .map(|_| eng.create_card(CardKind::Luminesce))
            .collect();
        let owner = eng.s.player.creature;
        eng.add_generated_cards_to_combat(&cards, PileType::Hand, Some(owner), Position::Bottom)
    }
}
