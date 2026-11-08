//! The cards' cost lists: `CardEnergyCost._localModifiers` and `CardModel._temporaryStarCosts`.
//!
//! They live beside the cards, not in [`Card`](super::state::Card), so that a card is `Copy`
//! and the card list clones as one copy (a search clones the state for every candidate action);
//! nearly every card has neither list. [`CombatState::energy_cost_mut`] and
//! [`CombatState::star_cost_mut`] add to them; the folds are [`EnergyCost::with_local_modifiers`]
//! and [`StarCost::current`].
//!
//! [`EnergyCost::with_local_modifiers`]: super::energy_cost::EnergyCost::with_local_modifiers
//! [`StarCost::current`]: super::star_cost::StarCost::current

use std::ops::Range;

use super::energy_cost::{CostModifier, CostType, Expiration};
use super::star_cost::TemporaryCost;
use super::state::{CardId, CombatState};

/// One kind of cost list for every card, flattened: the entries of all cards, ordered by card
/// and, within a card, in the order they were added. `cards[i]` is the card of `items[i]`, so a
/// card's list is one slice of `items`, and a copy is two `Copy` lists (no list per card to
/// allocate). The order is a function of each card's list, so the table is the same however
/// the lists were reached.
#[derive(Debug, PartialEq, Eq, Hash)]
struct CardLists<T> {
    cards: Vec<CardId>,
    items: Vec<T>,
}

impl<T> Default for CardLists<T> {
    fn default() -> Self {
        Self {
            cards: Vec::new(),
            items: Vec::new(),
        }
    }
}

impl<T: Copy> Clone for CardLists<T> {
    fn clone(&self) -> Self {
        Self {
            cards: self.cards.clone(),
            items: self.items.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        crate::copy_vec_from(&mut self.cards, &source.cards);
        crate::copy_vec_from(&mut self.items, &source.items);
    }
}

impl<T: Copy> CardLists<T> {
    const fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Where `card`'s entries are in `items` (empty, at their place, if it has none).
    fn range(&self, card: CardId) -> Range<usize> {
        let start = self.cards.partition_point(|&c| c < card);
        let len = self.cards[start..].partition_point(|&c| c == card);
        start..start + len
    }

    /// `card`'s list, in order.
    fn get(&self, card: CardId) -> &[T] {
        if self.is_empty() {
            return &[];
        }
        &self.items[self.range(card)]
    }

    /// `card`'s list, in order, to change in place.
    fn get_mut(&mut self, card: CardId) -> &mut [T] {
        let range = self.range(card);
        &mut self.items[range]
    }

    /// Appends `item` to `card`'s list.
    fn push(&mut self, card: CardId, item: T) {
        let at = self.cards.partition_point(|&c| c <= card);
        self.cards.insert(at, card);
        self.items.insert(at, item);
    }

    /// Keeps the entries for which `keep(card, item)` holds.
    fn retain(&mut self, mut keep: impl FnMut(CardId, &T) -> bool) {
        let mut kept = 0;
        for i in 0..self.cards.len() {
            let (card, item) = (self.cards[i], self.items[i]);
            if keep(card, &item) {
                self.cards[kept] = card;
                self.items[kept] = item;
                kept += 1;
            }
        }
        self.cards.truncate(kept);
        self.items.truncate(kept);
    }

    /// Gives `to` (a card without a list) a copy of `from`'s list.
    fn copy(&mut self, from: CardId, to: CardId) {
        for k in 0..self.range(from).len() {
            let item = self.items[self.range(from).start + k];
            self.push(to, item);
        }
    }
}

/// The cost lists of every card. A card without entries has two empty lists.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct CostLists {
    /// `_localModifiers`, each card's in the order they were added.
    energy: CardLists<CostModifier>,
    /// `_temporaryStarCosts`, each card's in the order they were added; the last one wins.
    star: CardLists<TemporaryCost>,
}

impl Clone for CostLists {
    fn clone(&self) -> Self {
        Self {
            energy: self.energy.clone(),
            star: self.star.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.energy.clone_from(&source.energy);
        self.star.clone_from(&source.star);
    }
}

impl CostLists {
    /// Whether no card has a cost list.
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.energy.is_empty() && self.star.is_empty()
    }

    /// `card`'s local energy cost modifiers, in order.
    #[must_use]
    #[inline]
    pub fn energy(&self, card: CardId) -> &[CostModifier] {
        self.energy.get(card)
    }

    /// `card`'s temporary star costs, in order.
    #[must_use]
    #[inline]
    pub fn star(&self, card: CardId) -> &[TemporaryCost] {
        self.star.get(card)
    }

    /// Gives `to` (a new card without lists) the lists of `from` (`CreateClone`).
    pub(crate) fn copy(&mut self, from: CardId, to: CardId) {
        self.energy.copy(from, to);
        self.star.copy(from, to);
    }

    /// `EndOfTurnCleanup` for the cards for which `in_pile` holds: drop the energy modifiers
    /// that expire at the end of the turn and the star costs that clear then.
    pub(crate) fn end_of_turn_cleanup(&mut self, mut in_pile: impl FnMut(CardId) -> bool) {
        if self.is_empty() {
            return;
        }
        self.energy
            .retain(|c, m| !(m.expiration.has(Expiration::END_OF_TURN) && in_pile(c)));
        self.star
            .retain(|c, t| !(t.clears_when_turn_ends && in_pile(c)));
    }

    /// `AfterCardPlayedCleanup` of both costs (`CardModel.cs:2011`): drop the energy modifiers
    /// that expire when the card is played and the star costs that clear then.
    pub(crate) fn after_card_played_cleanup(&mut self, card: CardId) {
        if self.is_empty() {
            return;
        }
        self.energy
            .retain(|c, m| c != card || !m.expiration.has(Expiration::WHEN_PLAYED));
        self.star.retain(|c, t| c != card || !t.clears_when_played);
    }

    /// The list half of `UpgradeBy` and `UpgradeStarCostBy`: an energy base lowered to `energy`
    /// caps every absolute modifier at it, a star base lowered to `star` drops every temporary
    /// cost above it.
    pub(crate) fn apply_upgrade(&mut self, card: CardId, energy: Option<i32>, star: Option<i32>) {
        if self.is_empty() {
            return;
        }
        if let Some(new) = energy {
            for m in self.energy.get_mut(card) {
                if m.kind == CostType::Absolute && m.amount > new {
                    m.amount = new;
                }
            }
        }
        if let Some(base) = star {
            self.star.retain(|c, t| c != card || t.cost <= base);
        }
    }
}

/// [`CombatState::energy_cost_mut`]: adds local modifiers to a card's energy cost
/// (`CardEnergyCost`'s `Set*`/`Add*` methods).
#[derive(Debug)]
pub struct EnergyCostMut<'a> {
    card: CardId,
    /// `Canonical` of the card's cost.
    canonical: i32,
    lists: &'a mut CostLists,
}

impl EnergyCostMut<'_> {
    fn add(&mut self, amount: i32, kind: CostType, expiration: Expiration, reduce_only: bool) {
        let modifier = CostModifier {
            amount,
            kind,
            expiration,
            reduce_only,
        };
        self.lists.energy.push(self.card, modifier);
    }

    /// `SetThisTurnOrUntilPlayed(cost, reduceOnly: true)`.
    pub fn reduce_this_turn_or_until_played(&mut self, cost: i32) {
        if cost != 0 || self.canonical >= 0 {
            let expiration = Expiration::END_OF_TURN.with(Expiration::WHEN_PLAYED);
            self.add(cost, CostType::Absolute, expiration, true);
        }
    }

    /// `SetThisCombat(cost, reduceOnly: true)`.
    pub fn reduce_this_combat(&mut self, cost: i32) {
        if cost != 0 || self.canonical >= 0 {
            self.add(cost, CostType::Absolute, Expiration::END_OF_COMBAT, true);
        }
    }

    /// `SetThisTurnOrUntilPlayed(cost)` (also `SetToFreeThisTurn` with 0).
    pub fn set_this_turn_or_until_played(&mut self, cost: i32) {
        if cost != 0 || self.canonical >= 0 {
            let expiration = Expiration::END_OF_TURN.with(Expiration::WHEN_PLAYED);
            self.add(cost, CostType::Absolute, expiration, false);
        }
    }

    /// `SetUntilPlayed(cost)`.
    pub fn set_until_played(&mut self, cost: i32) {
        if cost != 0 || self.canonical >= 0 {
            self.add(cost, CostType::Absolute, Expiration::WHEN_PLAYED, false);
        }
    }

    /// `SetThisTurn(cost)`.
    pub fn set_this_turn(&mut self, cost: i32) {
        if cost != 0 || self.canonical >= 0 {
            self.add(cost, CostType::Absolute, Expiration::END_OF_TURN, false);
        }
    }

    /// `SetThisCombat(cost)` (also `SetToFreeThisCombat` with 0).
    pub fn set_this_combat(&mut self, cost: i32) {
        if cost != 0 || self.canonical >= 0 {
            self.add(cost, CostType::Absolute, Expiration::END_OF_COMBAT, false);
        }
    }

    /// `AddUntilPlayed(amount)`.
    pub fn add_until_played(&mut self, amount: i32) {
        if amount != 0 {
            self.add(amount, CostType::Relative, Expiration::WHEN_PLAYED, false);
        }
    }

    /// `AddThisTurnOrUntilPlayed(amount)`.
    pub fn add_this_turn_or_until_played(&mut self, amount: i32) {
        if amount != 0 {
            let expiration = Expiration::END_OF_TURN.with(Expiration::WHEN_PLAYED);
            self.add(amount, CostType::Relative, expiration, false);
        }
    }

    /// `AddThisTurn(amount)`.
    pub fn add_this_turn(&mut self, amount: i32) {
        if amount != 0 {
            self.add(amount, CostType::Relative, Expiration::END_OF_TURN, false);
        }
    }

    /// `AddThisCombat(amount)`.
    pub fn add_this_combat(&mut self, amount: i32) {
        if amount != 0 {
            self.add(amount, CostType::Relative, Expiration::END_OF_COMBAT, false);
        }
    }
}

/// [`CombatState::star_cost_mut`]: adds temporary star costs to a card (`CardModel`'s
/// `SetStarCost*` methods).
#[derive(Debug)]
pub struct StarCostMut<'a> {
    card: CardId,
    lists: &'a mut CostLists,
}

impl StarCostMut<'_> {
    fn add(&mut self, cost: i32, clears_when_turn_ends: bool, clears_when_played: bool) {
        let cost = TemporaryCost {
            cost: cost.max(0),
            clears_when_turn_ends,
            clears_when_played,
        };
        self.lists.star.push(self.card, cost);
    }

    /// `SetStarCostUntilPlayed(cost)`.
    pub fn set_until_played(&mut self, cost: i32) {
        self.add(cost, false, true);
    }

    /// `SetStarCostThisTurn(cost)` (also `SetToFreeThisTurn` with 0).
    pub fn set_this_turn(&mut self, cost: i32) {
        self.add(cost, true, true);
    }

    /// `SetStarCostThisCombat(cost)` (also `SetToFreeThisCombat` with 0).
    pub fn set_this_combat(&mut self, cost: i32) {
        self.add(cost, false, false);
    }
}

impl CombatState {
    /// Adds local modifiers to `card`'s energy cost.
    pub fn energy_cost_mut(&mut self, card: CardId) -> EnergyCostMut<'_> {
        EnergyCostMut {
            card,
            canonical: self.card(card).energy_cost.canonical,
            lists: &mut self.cost_lists,
        }
    }

    /// Adds temporary star costs to `card`.
    pub const fn star_cost_mut(&mut self, card: CardId) -> StarCostMut<'_> {
        StarCostMut {
            card,
            lists: &mut self.cost_lists,
        }
    }

    /// The cards' cost lists.
    #[must_use]
    pub const fn cost_lists(&self) -> &CostLists {
        &self.cost_lists
    }

    /// `card`'s energy cost with its local modifiers ([`EnergyCost::with_local_modifiers`]).
    ///
    /// [`EnergyCost::with_local_modifiers`]: super::energy_cost::EnergyCost::with_local_modifiers
    pub(crate) fn local_energy_cost(&self, card: CardId) -> Result<i32, i32> {
        self.card(card)
            .energy_cost
            .with_local_modifiers(self.cost_lists.energy(card))
    }

    /// `CurrentStarCost` of `card` ([`StarCost::current`]).
    ///
    /// [`StarCost::current`]: super::star_cost::StarCost::current
    #[must_use]
    pub fn current_star_cost(&self, card: CardId) -> i32 {
        self.card(card)
            .star_cost
            .current(self.cost_lists.star(card))
    }

    /// `SetToFreeThisTurn` (`CardModel.cs:1267`): energy cost 0 this turn or until played, star
    /// cost 0 this turn.
    pub fn set_to_free_this_turn(&mut self, card: CardId) {
        self.energy_cost_mut(card).set_this_turn_or_until_played(0);
        self.star_cost_mut(card).set_this_turn(0);
    }

    /// `SetToFreeThisCombat` (`CardModel.cs:1273`): both costs 0 for the combat.
    pub fn set_to_free_this_combat(&mut self, card: CardId) {
        self.energy_cost_mut(card).set_this_combat(0);
        self.star_cost_mut(card).set_this_combat(0);
    }

    /// `EndOfTurnCleanup` of one card (`CardModel.cs:1611`), its cost lists included.
    #[cfg(test)]
    pub(crate) fn end_of_turn_cleanup_of(&mut self, card: CardId) {
        self.card_mut(card).end_of_turn_cleanup();
        self.cost_lists.end_of_turn_cleanup(|c| c == card);
    }

    /// `UpgradeInternal` of `card` (no hooks), with the cost lists' part of the upgrade.
    pub(crate) fn upgrade_card_internal(&mut self, card: CardId) {
        let (energy, star) = self.card_mut(card).upgrade_internal();
        self.cost_lists.apply_upgrade(card, energy, star);
    }
}
