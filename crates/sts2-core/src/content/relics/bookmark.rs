//! `Bookmark` (`Models.Relics/Bookmark.cs`): after the hand is flushed, a random retained card
//! that costs energy gets 1 cheaper until played.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;

pub struct Bookmark;

impl RelicModel for Bookmark {
    fn name(&self) -> &'static str {
        "Bookmark"
    }
}

#[sts2_macros::hooks]
impl Hooks for Bookmark {
    type Me = RelicId;

    fn after_flush(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _flushed: &[CardId],
        retained: &[CardId],
    ) -> Flow {
        // !CostsX && GetWithModifiers(Local) > 0.
        let options: Vec<CardId> = retained
            .iter()
            .copied()
            .filter(|&c| {
                let cost = &eng.s.card(c).energy_cost;
                !cost.costs_x && eng.s.local_energy_cost(c).unwrap_or_else(|base| base) > 0
            })
            .collect();
        if let Some(&card) = eng.s.rng[RunRngType::CombatCardSelection].next_item(&options) {
            eng.s.energy_cost_mut(card).add_until_played(-1);
        }
        Ok(())
    }
}
