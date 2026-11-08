//! `PunchOffEventEncounter` (`Models.Encounters/PunchOffEventEncounter.cs`): two `PunchConstruct`s,
//! each losing `NextInt(2, 10)` HP on entry (the first draw is the one that opens with Fast
//! Punch). Only `PunchOff` starts it (it is in no act's encounter list).

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, PunchConstruct};
use crate::rng::Rng;

/// `PunchOffEventEncounter`: `PunchOff`'s fight.
pub struct PunchOffEventEncounter;

impl EncounterModel for PunchOffEventEncounter {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let first = PunchConstruct {
            starts_with_fast_punch: true,
            starting_hp_reduction: rng.next_int_range(2, 10),
        };
        let second = PunchConstruct {
            starts_with_fast_punch: false,
            starting_hp_reduction: rng.next_int_range(2, 10),
        };
        vec![
            MonsterKind::PunchConstruct(first),
            MonsterKind::PunchConstruct(second),
        ]
    }
}
