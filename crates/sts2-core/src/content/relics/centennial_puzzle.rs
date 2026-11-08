//! `CentennialPuzzle` (`Models.Relics/CentennialPuzzle.cs`): the first time the owner loses HP in a
//! fight, draw 3 cards.

use crate::combat::hooks::{DamageEvent, Hooks};
use crate::combat::{CombatState, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(3)`.
const CARDS: i32 = 3;

pub struct CentennialPuzzle;

/// `CentennialPuzzle`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `UsedThisCombat`.
    pub used_this_combat: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::CentennialPuzzle(data) = &mut s.relic_mut(me).data else {
        unreachable!("a CentennialPuzzle has CentennialPuzzle data");
    };
    data
}

impl RelicModel for CentennialPuzzle {
    fn name(&self) -> &'static str {
        "Centennial Puzzle"
    }

    fn init_data(&self) -> RelicData {
        RelicData::CentennialPuzzle(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for CentennialPuzzle {
    type Me = RelicId;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: RelicId, hit: DamageEvent) -> Flow {
        if !eng.s.in_progress
            || hit.target != eng.s.player.creature
            || hit.result.unblocked <= 0
            || data_mut(eng.s, me).used_this_combat
        {
            return Ok(());
        }
        data_mut(eng.s, me).used_this_combat = true;
        for _ in 0..CARDS {
            eng.draw(Decimal::ONE, false)?;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).used_this_combat = false;
        Ok(())
    }
}
