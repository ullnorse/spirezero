//! `PlayerCmd.GainGold` and `PlayerCmd.LoseGold` (`PlayerCmd.cs:143-206`), on the combat's copy
//! of the player's gold; the run layer owns the total.

use crate::combat::engine::{Engine, Flow};
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `PlayerCmd.GainGold(amount)`: `ModifyGoldGained`, `AfterModifyingGoldGained`, then (for
    /// a positive amount) `Gold += (int)amount` and `AfterGoldGained`.
    pub(crate) fn gain_gold(&mut self, amount: Decimal) -> Flow {
        let (amount, modifiers) = self.hooked().modify_gold_gained(amount);
        self.after_modifying_gold_gained(&modifiers, amount)?;
        if amount <= Decimal::ZERO {
            return Ok(());
        }
        self.s.player.gold += amount.to_int_truncating();
        self.after_gold_gained()
    }

    /// `PlayerCmd.LoseGold(amount)`: `Gold = max(0, Gold - (int)amount)`; no hooks.
    pub(crate) fn lose_gold(&mut self, amount: Decimal) {
        let player = &mut self.s.player;
        player.gold = (player.gold - amount.to_int_truncating()).max(0);
    }
}
