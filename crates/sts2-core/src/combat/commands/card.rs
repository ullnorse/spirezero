//! Card commands beyond the piles (`CardCmd`): afflict, enchant, downgrade, transform, discard.

use crate::combat::AutoPlayType;
use crate::combat::engine::{Engine, Flow};
use crate::combat::history::HistoryKind;
use crate::combat::state::CardId;
use crate::content::afflictions::{Affliction, AfflictionKind};
use crate::content::cards::{self, CardKind};
use crate::content::enchantments::{self, Enchantment, EnchantmentKind};
use crate::decimal::Decimal;

use super::piles::Position;
use crate::combat::PileType;

impl Engine<'_> {
    /// `CardCmd.Afflict<T>(card, amount)` (`CardCmd.cs:617-665`): nothing for a combat-pile card
    /// once combat is over or ending, if a listener's `ShouldAfflict` vetoes (no in-scope
    /// listener), or if `CanAfflict` fails; else the new affliction (`AfflictInternal`, then
    /// `AfterApplied`, which no in-scope affliction overrides) or `+amount` on the same kind,
    /// then a `CardAfflicted` history entry.
    pub(crate) fn afflict(&mut self, card: CardId, kind: AfflictionKind, amount: Decimal) {
        if self.hooked().is_over_or_ending() && self.s.pile_of(card).is_some() {
            return;
        }
        if !kind.can_afflict_with(self.hooked(), card) {
            return;
        }
        let amount = amount.to_int_truncating();
        match self.s.card_mut(card).affliction.as_mut() {
            None => {
                self.s
                    .set_affliction(card, Some(Affliction { kind, amount }));
            }
            Some(existing) => existing.amount += amount,
        }
        let me = self.s.player.creature;
        self.record(
            me,
            HistoryKind::CardAfflicted {
                card,
                affliction: kind,
            },
        );
    }

    /// `CardCmd.ClearAffliction(card)` (`ClearAfflictionInternal`; `BeforeRemoved` has no
    /// in-scope override).
    pub(crate) fn clear_affliction(&mut self, card: CardId) {
        self.s.set_affliction(card, None);
    }

    /// `CardCmd.Enchant<T>(card, amount)` (`CardCmd.cs:536-562`): a new enchantment
    /// (`EnchantInternal`, then `ModifyCard`: `OnEnchant`) or `+amount` on the same kind.
    ///
    /// # Panics
    /// If `CanEnchant` fails, like the game.
    pub(crate) fn enchant(&mut self, card: CardId, kind: EnchantmentKind, amount: Decimal) {
        assert!(
            kind.can_enchant(self.s.card(card)),
            "cannot enchant {:?} with {kind:?}",
            self.s.card(card).kind
        );
        let amount = amount.to_int_truncating();
        match self.s.card_mut(card).enchantment.as_mut() {
            None => {
                let c = self
                    .s
                    .set_enchantment(card, Some(Enchantment::new(kind, amount)));
                enchantments::modify_card(c);
            }
            Some(existing) => existing.amount += amount,
        }
    }

    /// `CardCmd.Downgrade(card)` (`CardCmd.cs:212-224`, `DowngradeInternal`): unless combat is
    /// ending, back to upgrade level 0 with the canonical keywords and cost (`ResetForDowngrade`)
    /// but the same affliction, enchantment and model fields, then `AfterDowngraded`, the
    /// enchantment's `ModifyCard` (`OnEnchant` re-adds its keywords) and the affliction's
    /// `AfterApplied` (no in-scope effect). A calculated var keeps the upgraded base as its
    /// `BaseValue` unless the enchantment recalculates it ([`Card::stale_calculated`](crate::combat::state::Card::stale_calculated)).
    pub(crate) fn downgrade(&mut self, card: CardId) {
        if self.hooked().is_ending() {
            return;
        }
        let c = self.s.card_mut(card);
        let old_base = c.calculation_base();
        c.upgrade_level = 0;
        c.keep_stale_calculated(old_base);
        c.keywords = c.kind.keywords();
        c.energy_cost.reset_for_downgrade();
        c.star_cost.reset_for_downgrade();
        cards::after_downgraded(self.s, card);
        enchantments::modify_card(self.s.card_mut(card));
    }

    /// `CardCmd.TransformTo<T>(original)` for one card (`CardCmd.cs:337-520`): nothing once
    /// combat is ending; else a new `kind` card (`CreateCard`, with `AfterCreated`) takes the
    /// original's place in its pile (`AddInternal` at its index), with a `CardGenerated` entry,
    /// `AfterCardEnteredCombat` and `AfterCardChangedPiles`; then
    /// `AfterCardGeneratedForCombat`, and the original leaves the combat. No in-scope card
    /// overrides `AfterTransformedFrom/To`.
    pub(crate) fn transform_to(&mut self, original: CardId, kind: CardKind) -> Flow {
        if self.hooked().is_ending() {
            return Ok(());
        }
        let pile = self
            .s
            .pile_of(original)
            .expect("a transformed card is in a pile");
        let replacement = self.create_card(kind);
        let index = self
            .s
            .player
            .pile(pile)
            .iter()
            .position(|&c| c == original)
            .expect("the card is in its pile");
        self.s.remove_from_pile(original);
        self.s.insert_into_pile(replacement, pile, Some(index));
        let me = self.s.player.creature;
        self.record(
            me,
            HistoryKind::CardGenerated {
                card: replacement,
                creator: Some(me),
            },
        );
        self.after_card_entered_combat(replacement)?;
        // The old pile passed is the replacement's own pile (`CardCmd.cs:451`).
        self.after_card_changed_piles(replacement, Some(pile))?;
        if self.s.pile_of(replacement).is_some() {
            self.after_card_generated_for_combat(replacement, Some(me))?;
        }
        Ok(())
    }

    /// `CardCmd.Discard(cards)` (`DiscardAndDraw` with no draw, `CardCmd.cs:172-209`): nothing
    /// once combat is over or ending; else each card to the discard pile in turn, with a
    /// `CardDiscarded` entry and `AfterCardDiscarded`, then every card that was Sly this turn
    /// is autoplayed (`AutoPlayType.SlyDiscard`).
    pub(crate) fn discard(&mut self, cards: &[CardId]) -> Flow {
        self.discard_and_draw(cards, 0)
    }

    /// `CardCmd.DiscardAndDraw(cards, draw)` (`CardCmd.cs:172-205`): as [`Engine::discard`],
    /// with `draw` cards drawn after the discards and before the Sly autoplays.
    pub(crate) fn discard_and_draw(&mut self, cards: &[CardId], draw: i32) -> Flow {
        if self.hooked().is_over_or_ending() || cards.is_empty() {
            return Ok(());
        }
        let mut sly = Vec::new();
        for &card in cards {
            if self.hooked().is_sly_this_turn(card) {
                sly.push(card);
            }
            self.add_to_pile(card, PileType::Discard, Position::Bottom)?;
            let me = self.s.player.creature;
            self.record(me, HistoryKind::CardDiscarded(card));
            self.after_card_discarded(card)?;
        }
        if draw > 0 {
            self.draw(Decimal::from(draw), false)?;
        }
        for card in sly {
            self.auto_play_with(card, None, AutoPlayType::SlyDiscard, false)?;
        }
        Ok(())
    }
}
