//! `GremlinMercNormal` (`Models.Encounters/GremlinMercNormal.cs`): a `GremlinMerc` in the `merc`
//! slot (the encounter has no `Slots` list). When it dies, `SurprisePower` adds a `SneakyGremlin`
//! (`sneaky`) and a `FatGremlin` (`fat`) holding the stolen gold. The gold reward is full unless the
//! fat gremlin escaped: half if the merc stole nothing, none if it did (`CalculateGoldProportion`).

use super::{EncounterData, EncounterModel};
use crate::combat::CombatState;
use crate::content::monsters::{GremlinMerc, MonsterKind};
use crate::rng::Rng;
use crate::run::odds::GoldProportion;

/// `GremlinMercNormal`: a `GremlinMerc`.
pub struct GremlinMercNormal;

impl EncounterModel for GremlinMercNormal {
    fn spawn_slots(&self) -> &'static [&'static str] {
        &["merc"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::GremlinMerc(GremlinMerc)]
    }

    fn calculate_gold_proportion(&self, s: &CombatState) -> GoldProportion {
        let fat_gremlin_escaped = s.escaped.iter().any(|&c| {
            matches!(
                s.creature(c).monster.as_ref().map(|m| m.kind),
                Some(MonsterKind::FatGremlin(_))
            )
        });
        if !fat_gremlin_escaped {
            GoldProportion::ONE
        } else if s.encounter_data
            == (EncounterData::GremlinMercNormal {
                gold_was_stolen: true,
            })
        {
            GoldProportion::ZERO
        } else {
            GoldProportion::HALF
        }
    }
}
