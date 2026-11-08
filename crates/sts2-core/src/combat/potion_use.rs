//! The potion-use pipeline: `UsePotionAction.ExecuteAction` and `PotionModel.OnUseWrapper`
//! (`UsePotionAction.cs:104-128`, `PotionModel.cs:297-354`).

use super::engine::{Engine, Event, Flow};
use super::state::CreatureId;
use crate::content::potions;

impl Engine<'_> {
    /// A manual potion use, from emptying the slot to the executor's `CheckWinCondition`. The
    /// caller has checked that the slot holds a potion and that `target` is valid for it
    /// (`IsValidTarget`).
    pub(crate) fn use_potion_action(&mut self, slot: usize, target: Option<CreatureId>) -> Flow {
        let potion = self.s.player.potions[slot].expect("the slot holds a potion");
        // RemoveBeforeUse: the slot empties, the other potions keep theirs.
        self.s.set_potion(slot, None);
        self.emit(Event::PotionUsed {
            potion: potion.kind,
            target,
        });
        self.before_potion_used(potion, target)?;

        self.s.player.effect_depth += 1;
        let used = potions::on_use(self, potion.kind, target);
        self.s.player.effect_depth -= 1;
        used?;
        // EndCardOrPotionEffect -> RemoveDeadPlayerCardsFromCombat: only matters for a dead
        // player, whose combat is then lost.

        if !self.player_dead() {
            self.after_potion_used(potion, target)?;
            self.check_for_empty_hand()?;
        }
        self.check_win_condition()?;
        Ok(())
    }
}
