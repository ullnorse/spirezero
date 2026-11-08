//! `PlayerCmd.GainEnergy` and `LoseEnergy` (`PlayerCmd.cs:29-61`).

use crate::combat::engine::{Engine, Flow};
use crate::decimal::Decimal;

impl Engine<'_> {
    #[expect(
        clippy::unnecessary_wraps,
        reason = "mirrors an awaited command; its hooks can suspend once content listens"
    )]
    /// `PlayerCmd.GainEnergy(amount, player)` for the only player: nothing for a non-positive
    /// amount or once combat is ending; else `PlayerCombatState.GainEnergy`, which clamps to
    /// `0..=999999999` and truncates.
    pub(crate) fn gain_energy(&mut self, amount: Decimal) -> Flow {
        if amount <= Decimal::ZERO || self.hooked().is_ending() {
            return Ok(());
        }
        // Hook.ModifyEnergyGain, AfterModifyingEnergyGain: no in-scope listener.
        let final_amount = amount;
        if final_amount > Decimal::ZERO {
            let energy = &mut self.s.player.energy;
            *energy = (Decimal::from(*energy) + final_amount)
                .clamp(Decimal::ZERO, Decimal::from(999_999_999))
                .to_int_truncating();
        }
        Ok(())
    }

    /// `PlayerCmd.LoseEnergy(amount, player)` for the only player (`PlayerCmd.cs:49-61`):
    /// nothing for a non-positive amount or once combat is ending; else
    /// `PlayerCombatState.LoseEnergy`, which clamps to `0..=999999999` and truncates. No hooks.
    pub(crate) fn lose_energy(&mut self, amount: Decimal) {
        if amount <= Decimal::ZERO || self.hooked().is_ending() {
            return;
        }
        let energy = &mut self.s.player.energy;
        *energy = (Decimal::from(*energy) - amount)
            .clamp(Decimal::ZERO, Decimal::from(999_999_999))
            .to_int_truncating();
    }
}
