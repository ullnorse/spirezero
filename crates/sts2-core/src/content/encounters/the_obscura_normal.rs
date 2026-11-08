//! `TheObscuraNormal` (`Models.Encounters/TheObscuraNormal.cs`): one `TheObscura` in the
//! `obscura` slot. Its `Parafright` joins later in the `illusion` slot, which `Slots` puts first.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, TheObscura};
use crate::rng::Rng;

/// `TheObscuraNormal`: a `TheObscura`, which summons a `Parafright`.
pub struct TheObscuraNormal;

impl EncounterModel for TheObscuraNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["illusion", "obscura"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["obscura"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::TheObscura(TheObscura)]
    }
}
