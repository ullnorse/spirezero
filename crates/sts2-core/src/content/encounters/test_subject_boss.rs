//! `TestSubjectBoss` (`Models.Encounters/TestSubjectBoss.cs`): one `TestSubject`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, TestSubject};
use crate::rng::Rng;

/// `TestSubjectBoss`.
pub struct TestSubjectBoss;

impl EncounterModel for TestSubjectBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::TestSubject(TestSubject::default())]
    }
}
