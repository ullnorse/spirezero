//! `OvicopterNormal` (`Models.Encounters/OvicopterNormal.cs`): one `Ovicopter` in the
//! `ovicopter` slot; its `ToughEgg`s fill `egg5` down to `egg1`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Ovicopter};
use crate::rng::Rng;

/// `OvicopterNormal`: an `Ovicopter`, which lays `ToughEgg`s.
pub struct OvicopterNormal;

impl EncounterModel for OvicopterNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["egg1", "egg2", "egg3", "egg4", "egg5", "ovicopter"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["ovicopter"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Ovicopter(Ovicopter)]
    }
}
