//! Game content, one module per entity so content can be ported in parallel.
//!
//! Each `mod.rs` holds the kind enum and the dispatch from hook calls to the entity modules.

pub mod afflictions;
pub mod ancients;
pub mod cards;
pub mod characters;
pub mod enchantments;
pub mod encounters;
pub mod events;
mod id_table;
pub mod monsters;
pub mod potions;
pub mod powers;
pub mod relics;
pub mod tables;

pub(crate) use id_table::IdTable;

#[cfg(test)]
mod flags;

#[cfg(test)]
mod tests {
    use super::afflictions::AfflictionKind;
    use super::cards::CardKind;
    use super::enchantments::EnchantmentKind;
    use super::encounters::Encounter;
    use super::events::EventKind;
    use super::potions::PotionKind;
    use super::powers::PowerKind;
    use super::relics::RelicKind;

    /// `afflictions/test_dummy.rs` and `powers/test_dummy.rs` are only files: the build script
    /// gives them a variant, an id and a model, and leaves them out of `ALL` as test-only.
    #[test]
    fn new_files_register() {
        assert_eq!(AfflictionKind::TestDummy.id(), "TEST_DUMMY");
        assert_eq!(AfflictionKind::TestDummy.name(), "Test Dummy");
        assert!(!AfflictionKind::ALL.contains(&AfflictionKind::TestDummy));
        assert_eq!(AfflictionKind::from_id("TEST_DUMMY"), None);

        assert_eq!(PowerKind::TestDummy.id(), "TEST_DUMMY_POWER");
        assert_eq!(PowerKind::TestDummy.name(), "Test Dummy");
        assert!(!PowerKind::ALL.contains(&PowerKind::TestDummy));
        assert_eq!(PowerKind::from_id("TEST_DUMMY_POWER"), None);
    }

    /// Ids are unique per kind and `from_id` inverts `id`.
    #[test]
    fn ids_round_trip() {
        fn check<K: Copy + PartialEq + std::fmt::Debug>(
            all: &[K],
            id: impl Fn(K) -> &'static str,
            from_id: impl Fn(&str) -> Option<K>,
        ) {
            for &k in all {
                assert_eq!(from_id(id(k)), Some(k), "{k:?}");
            }
        }
        check(
            &AfflictionKind::ALL,
            AfflictionKind::id,
            AfflictionKind::from_id,
        );
        check(&CardKind::ALL, CardKind::id, CardKind::from_id);
        check(
            &EnchantmentKind::ALL,
            EnchantmentKind::id,
            EnchantmentKind::from_id,
        );
        check(&Encounter::ALL, Encounter::id, Encounter::from_id);
        check(&EventKind::ALL, EventKind::id, EventKind::from_id);
        check(&PotionKind::ALL, PotionKind::id, PotionKind::from_id);
        check(&PowerKind::ALL, PowerKind::id, PowerKind::from_id);
        check(&RelicKind::ALL, RelicKind::id, RelicKind::from_id);
    }
}
