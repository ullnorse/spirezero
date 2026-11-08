//! `ToadpolesWeak` (`Models.Encounters/ToadpolesWeak.cs`): two `Toadpole`s, the first with
//! `IsFront` set.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Toadpole};
use crate::rng::Rng;

/// `ToadpolesWeak`: a front and a back `Toadpole`.
pub struct ToadpolesWeak;

impl EncounterModel for ToadpolesWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::Toadpole(Toadpole { is_front: true }),
            MonsterKind::Toadpole(Toadpole { is_front: false }),
        ]
    }
}
