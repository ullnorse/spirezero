//! `Hoarder` (`Models.Modifiers/Hoarder.cs`): every card that joins the deck brings two copies
//! (added to the bottom, themselves not copied again), and the merchant removes no cards.

use crate::run::{RunCard, RunState, hooks};

/// `AfterCardChangedPiles` for a card that came from no pile into the deck: two clones, each
/// added like any card (`ModifyCardBeingAddedToDeck`, the relics' hooks) but skipped by this
/// modifier.
pub(super) fn after_card_added_to_deck(s: &mut RunState, card: RunCard) {
    for _ in 0..2 {
        let clone = hooks::modify_card_being_added_to_deck(s, card);
        s.deck.push(clone);
        hooks::after_card_added_to_deck_by_relics(s, clone);
    }
}
