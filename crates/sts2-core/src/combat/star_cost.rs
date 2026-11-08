//! A card instance's star cost and `LastStarsSpent`.
//!
//! Ports `BaseStarCost`, `_temporaryStarCosts` and `LastStarsSpent`
//! (`Models/CardModel.cs:410-490,1279-1330`), with `TemporaryCardCost`
//! (`Entities.Cards/TemporaryCardCost.cs`). The global part (`Hook.ModifyStarCost`) is folded in
//! [`CombatState::star_cost`].
//!
//! [`CombatState::star_cost`]: crate::combat::CombatState::star_cost

/// `TemporaryCardCost`: a cost the card is set to for a while.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TemporaryCost {
    /// `Cost`: never negative (`Math.Max(cost, 0)`).
    pub cost: i32,
    /// `ClearsWhenTurnEnds`.
    pub clears_when_turn_ends: bool,
    /// `ClearsWhenCardIsPlayed`.
    pub clears_when_played: bool,
}

/// The per-instance part of a card's star cost. Its `_temporaryStarCosts` are in the combat's
/// [`CostLists`](super::cost_lists::CostLists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StarCost {
    /// `CanonicalStarCost`: -1 for a card without a star cost.
    pub canonical: i32,
    /// `HasStarCostX`: the play spends all stars.
    pub costs_x: bool,
    /// `BaseStarCost`: the canonical cost after upgrades.
    pub base: i32,
    /// `LastStarsSpent`: the stars paid for (or, auto-played, valued at) the last play.
    pub last_spent: i32,
    /// The lowest base [`StarCost::upgrade_by`] lowered the cost to since
    /// [`Card::upgrade_internal`](super::state::Card::upgrade_internal) last took it: temporary
    /// costs above it are dropped. `None` between upgrades.
    pub(crate) upgrade_cap: Option<i32>,
}

impl StarCost {
    /// A fresh card's star cost.
    #[must_use]
    pub const fn new(canonical: i32, costs_x: bool) -> Self {
        Self {
            canonical,
            costs_x,
            base: canonical,
            last_spent: 0,
            upgrade_cap: None,
        }
    }

    /// `CurrentStarCost` (`CardModel.cs:451-466`) with the card's `temporary` costs: the last
    /// temporary cost, except that a card without a star cost set to 0 keeps having none; else
    /// the base.
    #[must_use]
    pub const fn current(&self, temporary: &[TemporaryCost]) -> i32 {
        match temporary.last() {
            Some(t) if t.cost == 0 && self.base < 0 => self.base,
            Some(t) => t.cost,
            None => self.base,
        }
    }

    /// `UpgradeStarCostBy(addend)` (call it from `on_upgrade`; `CardModel.cs:1311-1328`): the
    /// base moves by `addend` (no floor), and a lower base drops every temporary cost above it
    /// (applied by
    /// [`CombatState::upgrade_card_internal`](super::state::CombatState::upgrade_card_internal)).
    ///
    /// # Panics
    /// For an X star cost, as the game throws.
    pub fn upgrade_by(&mut self, addend: i32) {
        assert!(
            !self.costs_x,
            "UpgradeStarCostBy on a card with star cost X"
        );
        if addend == 0 {
            return;
        }
        let old = self.base;
        self.base += addend;
        if self.base < old {
            let base = self.base;
            self.upgrade_cap = Some(self.upgrade_cap.map_or(base, |cap| cap.min(base)));
        }
    }

    /// `DowngradeInternal`: the base back to the canonical cost; temporary costs stay.
    pub(crate) const fn reset_for_downgrade(&mut self) {
        if !self.costs_x {
            self.base = self.canonical;
        }
    }
}
