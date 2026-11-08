//! Afflictions (`AfflictionModel`): combat-only modifiers attached to a card. Each listens to
//! hooks right after its card (`CombatState.IterateHookListeners`); `Me` is the card's id.
//!
//! To add an affliction: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates
//! the variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").
//! `AfterApplied`, `BeforeRemoved` and `OnPlay` have no in-scope override.

include!(concat!(env!("OUT_DIR"), "/content/afflictions.rs"));

use crate::combat::CardId;
use crate::combat::hooks::{Hooked, Hooks};
use crate::combat::state::CombatState;
use crate::content::cards::{CardKeyword, CardType};

/// An affliction on a card (`CardModel.Affliction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Affliction {
    /// The affliction.
    pub kind: AfflictionKind,
    /// `Amount`.
    pub amount: i32,
}

/// An affliction's static data (`AfflictionModel`).
pub(crate) trait AfflictionModel: Hooks<Me = CardId> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English display name (`Title`).
    fn name(&self) -> &'static str;
    /// `CanAfflictCardType`.
    fn can_afflict_card_type(&self, _card_type: CardType) -> bool {
        true
    }
    /// `CanAfflictUnplayableCards`.
    fn can_afflict_unplayable_cards(&self) -> bool {
        true
    }
    /// `IsStackable`.
    fn is_stackable(&self) -> bool {
        false
    }
}

impl AfflictionKind {
    /// The English display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        with_model!(self, m => m.name())
    }

    /// Whether the model can raise a player choice (see `may_prompt` in the content API),
    /// statically dispatched: [`CombatState::may_prompt`](crate::combat::CombatState::may_prompt)
    /// asks every model on every step.
    #[must_use]
    pub fn may_prompt(self) -> bool {
        with_model!(self, m => m.may_prompt())
    }

    /// `CanAfflict(card)` (`AfflictionModel.cs`): the card type, Unplayable, and no other
    /// affliction unless this one stacks onto the same kind.
    #[must_use]
    pub fn can_afflict(self, s: &CombatState, id: CardId) -> bool {
        self.can_afflict_with(s.hooked(), id)
    }

    /// [`AfflictionKind::can_afflict`] on [`Hooked`].
    pub(crate) fn can_afflict_with(self, h: Hooked<'_>, id: CardId) -> bool {
        let m = self.model();
        let card = h.s.card(id);
        m.can_afflict_card_type(card.card_type())
            && (!h.has_keyword(id, CardKeyword::Unplayable) || m.can_afflict_unplayable_cards())
            && card
                .affliction
                .is_none_or(|a| m.is_stackable() && a.kind == self)
    }
}
