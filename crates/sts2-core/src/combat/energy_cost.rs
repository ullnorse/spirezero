//! A card instance's energy cost and local cost modifiers.
//!
//! Ports `Entities.Cards/CardEnergyCost.cs` and `Entities.Cards/LocalCostModifier.cs`. The
//! global part of the cost (the `TryModifyEnergyCostInCombat` hooks) is folded in
//! [`CombatState::energy_cost`].
//!
//! [`CombatState::energy_cost`]: crate::combat::CombatState::energy_cost

/// `LocalCostType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CostType {
    /// The cost becomes the amount.
    Absolute,
    /// The amount is added to the cost.
    Relative,
}

/// `LocalCostModifierExpiration` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Expiration(u8);

impl Expiration {
    /// `EndOfTurn`.
    pub const END_OF_TURN: Self = Self(1);
    /// `WhenPlayed`.
    pub const WHEN_PLAYED: Self = Self(1 << 1);
    /// `EndOfCombat`.
    pub const END_OF_COMBAT: Self = Self(1 << 2);

    /// Both sets of flags.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `HasFlag`.
    #[must_use]
    pub const fn has(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }
}

/// `LocalCostModifier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CostModifier {
    /// `Amount`.
    pub amount: i32,
    /// `Type`.
    pub kind: CostType,
    /// `Expiration`.
    pub expiration: Expiration,
    /// `IsReduceOnly`: never raises the cost.
    pub reduce_only: bool,
}

impl CostModifier {
    /// `Modify(currentCost)`.
    #[must_use]
    pub fn modify(&self, cost: i32) -> i32 {
        match (self.kind, self.reduce_only) {
            (CostType::Absolute, false) => self.amount,
            (CostType::Absolute, true) => cost.min(self.amount),
            (CostType::Relative, false) => cost + self.amount,
            (CostType::Relative, true) => cost.min(cost + self.amount),
        }
    }
}

/// `CardEnergyCost`: the per-instance part of a card's energy cost. Its `_localModifiers` are
/// in the combat's [`CostLists`](super::cost_lists::CostLists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnergyCost {
    /// `Canonical`: the printed cost (0 for X costs, negative for unplayable cards).
    pub canonical: i32,
    /// `CostsX`.
    pub costs_x: bool,
    /// `_base`: the canonical cost after upgrades and `SetCustomBaseCost`.
    pub base: i32,
    /// `CapturedXValue`: the energy spent on the last play of an X-cost card.
    pub captured_x: i32,
    /// The lowest base [`EnergyCost::upgrade_by`] lowered the cost to since
    /// [`Card::upgrade_internal`](super::state::Card::upgrade_internal) last took it: the cap of
    /// the absolute modifiers. `None` between upgrades.
    pub(crate) upgrade_cap: Option<i32>,
}

impl EnergyCost {
    /// `new CardEnergyCost(card, canonicalCost, costsX)`.
    #[must_use]
    pub const fn new(canonical: i32, costs_x: bool) -> Self {
        let canonical = if costs_x { 0 } else { canonical };
        Self {
            canonical,
            costs_x,
            base: canonical,
            captured_x: 0,
            upgrade_cap: None,
        }
    }

    /// `GetWithModifiers(CostModifiers.Local)` before the global hooks and the final
    /// `max(0, ..)`, with the card's local `modifiers`: `Ok` with them applied, or `Err(base)`
    /// when the base is returned as is (a negative base or an X cost).
    pub(crate) fn with_local_modifiers(&self, modifiers: &[CostModifier]) -> Result<i32, i32> {
        if self.base < 0 || self.costs_x {
            return Err(self.base);
        }
        Ok(modifiers.iter().fold(self.base, |cost, m| m.modify(cost)))
    }

    /// `ResetForDowngrade`: the base back to the canonical cost; modifiers stay.
    pub(crate) const fn reset_for_downgrade(&mut self) {
        self.base = self.canonical;
    }

    /// `UpgradeBy(addend)` (call it from `on_upgrade`): the base moves by `addend` (not below
    /// 0), and a lower base caps every absolute modifier (applied by
    /// [`CombatState::upgrade_card_internal`](super::state::CombatState::upgrade_card_internal)).
    pub fn upgrade_by(&mut self, addend: i32) {
        if self.costs_x || addend == 0 {
            return;
        }
        let old = self.base;
        let new = (self.base + addend).max(0);
        if new < old {
            self.upgrade_cap = Some(self.upgrade_cap.map_or(new, |cap| cap.min(new)));
        }
        self.base = new;
    }
}
