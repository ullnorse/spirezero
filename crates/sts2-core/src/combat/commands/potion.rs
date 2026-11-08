//! `PotionCmd.TryToProcure` and `PotionCmd.Discard` (`PotionCmd.cs`).

use crate::combat::engine::{Engine, Flow};
use crate::combat::state::{Potion, PotionId};
use crate::content::potions::PotionKind;

impl Engine<'_> {
    /// `PotionCmd.TryToProcure<T>(player)` with no slot index (`PotionCmd.cs:28-47`):
    /// `Hook.ShouldProcurePotion` (`Sozu`), then `AddPotionInternal` into the first empty slot,
    /// then `AfterPotionProcured`. Returns whether the potion was procured.
    pub(crate) fn try_to_procure_potion(&mut self, kind: PotionKind) -> Flow<bool> {
        if !self.hooked().should_procure_potion() {
            return Ok(false);
        }
        let Some(slot) = self.s.player.first_empty_potion_slot() else {
            return Ok(false);
        };
        let potion = Potion {
            id: PotionId(self.s.next_potion_id),
            kind,
        };
        self.s.next_potion_id += 1;
        self.s.set_potion(slot, Some(potion));
        self.after_potion_procured(potion)?;
        Ok(true)
    }

    /// `PotionCmd.Discard(potion)` (`PotionCmd.cs:50-60`) for the potion in `slot`: the slot
    /// empties (`DiscardPotionInternal`), then `AfterPotionDiscarded`.
    pub(crate) fn discard_potion(&mut self, slot: usize) -> Flow {
        let potion = self.s.player.potions[slot].expect("a discarded potion is in its slot");
        self.s.set_potion(slot, None);
        self.after_potion_discarded(potion)
    }
}
