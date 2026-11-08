//! Run-level relics: the relics the player owns, with their fields, and the bridge to the
//! combat engine.
//!
//! Every owned relic is a ported [`RelicKind`]: its combat hooks run in the combat engine and its
//! run-level hooks ([`RelicModel`](crate::content::relics::RelicModel)'s run section) are
//! dispatched by the run layer (`run/hooks.rs`). Obtaining any other relic is
//! [`RunError::UnsupportedRelic`](super::RunError::UnsupportedRelic).

use super::relic_bag::{BAG_RARITIES, SHARED_RELIC_POOL};
use crate::combat::state::Relic;
use crate::content::characters::Character;
use crate::content::relics::{OwnedRelic, RelicData, RelicKind};

/// A relic the player owns, with its fields (`[SavedProperty]` and private).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RunRelic {
    /// The relic.
    pub kind: RelicKind,
    /// Its fields.
    pub data: RelicData,
    /// `IsWax` (`ToyBox`'s relics): the relic works until it is melted.
    pub wax: bool,
    /// `IsMelted` (`RelicCmd.Melt`): the relic stays owned, and `GetRelic` still finds it, but
    /// it is no hook listener (`RunState.IterateHookListeners`, `CombatState.IterateHookListeners`)
    /// and not tradable.
    pub melted: bool,
}

impl RunRelic {
    /// A freshly obtained relic.
    #[must_use]
    pub fn new(kind: RelicKind) -> Self {
        Self {
            kind,
            data: kind.init_data(),
            wax: false,
            melted: false,
        }
    }

    /// `ModelId.Entry`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.kind.id()
    }
}

/// The relics as the combat receives them: every owned relic, in order, with its fields (the
/// game's combat listens to all of them).
#[must_use]
pub fn combat_relics(relics: &[RunRelic]) -> Vec<OwnedRelic> {
    relics
        .iter()
        .map(|r| OwnedRelic {
            kind: r.kind,
            data: r.data,
            melted: r.melted,
        })
        .collect()
}

/// Writes the fields of the combat's relics back into `relics` after a fight; `combat` is the
/// fight's relic list, built from [`combat_relics`] of `relics`.
///
/// # Panics
/// If `combat` does not match `relics`.
pub fn write_back(relics: &mut [RunRelic], combat: &[Relic]) {
    assert_eq!(
        relics.len(),
        combat.len(),
        "the fight's relics are the run's"
    );
    for (relic, fought) in relics.iter_mut().zip(combat) {
        assert_eq!(fought.kind, relic.kind, "the fight's relics are the run's");
        relic.data = fought.data;
    }
}

/// The relics of a tradable rarity whose `HasUponPickupEffect` or `SpawnsPets` is true
/// (`Models.Relics/*.cs`).
pub(crate) const UPON_PICKUP_OR_PETS: [&str; 15] = [
    "CAULDRON",
    "DOLLYS_MIRROR",
    "GNARLED_HAMMER",
    "KIFUDA",
    "LEES_WAFFLE",
    "MANGO",
    "OLD_COIN",
    "ORRERY",
    "PEAR",
    "POTION_BELT",
    "PUNCH_DAGGER",
    "ROYAL_STAMP",
    "STRAWBERRY",
    "WAR_PAINT",
    "WHETSTONE",
];

/// `RelicModel.Rarity` is not `Starter`, `Event` or `Ancient`: a Common, Uncommon, Rare or Shop
/// relic of the shared or a character pool, or a `None` one (`Circlet`, `DeprecatedRelic`).
fn has_tradable_rarity(id: &str) -> bool {
    matches!(id, "CIRCLET" | "DEPRECATED_RELIC")
        || Character::ALL
            .iter()
            .flat_map(|c| c.relic_pool())
            .chain(&SHARED_RELIC_POOL)
            .any(|(r, rarity)| *r == id && BAG_RARITIES.contains(rarity))
}

/// `RelicModel.IsTradable` (`Models/RelicModel.cs:176-202`): not used up, no upon-pickup effect,
/// not melted, no pets, and not a Starter, Event or Ancient relic.
#[must_use]
pub fn is_tradable(relic: &RunRelic) -> bool {
    !relic.kind.is_used_up(&relic.data)
        && !relic.melted
        && !UPON_PICKUP_OR_PETS.contains(&relic.id())
        && has_tradable_rarity(relic.id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::RelicId;
    use crate::content::relics::nunchaku;

    #[test]
    fn combat_relic_data_round_trips_through_the_run() {
        // Nunchaku.cs: `[SavedProperty] AttacksPlayed` outlives the combat.
        let mut relics = vec![
            RunRelic::new(RelicKind::BoundPhylactery),
            RunRelic::new(RelicKind::GoldenPearl),
            RunRelic::new(RelicKind::Nunchaku),
        ];
        let fresh = RelicData::Nunchaku(nunchaku::Data { attacks_played: 0 });
        assert_eq!(relics[0].data, RelicData::None);
        assert_eq!(relics[2].data, fresh);
        let owned = combat_relics(&relics);
        assert_eq!(owned.len(), 3);
        let played = RelicData::Nunchaku(nunchaku::Data { attacks_played: 7 });
        let fought: Vec<Relic> = owned
            .iter()
            .zip(0..)
            .map(|(r, i)| Relic {
                id: RelicId(i),
                kind: r.kind,
                data: if r.kind == RelicKind::Nunchaku {
                    played
                } else {
                    r.data
                },
                melted: false,
            })
            .collect();
        write_back(&mut relics, &fought);
        assert_eq!(relics[2].data, played);
        assert_eq!(combat_relics(&relics)[2].data, played);
    }

    #[test]
    fn tradable_rarities_match_the_game() {
        // `RelicRarity` Starter, Event and Ancient are not tradable; every other relic is.
        let table = include_str!("../content/flags/relics.tsv");
        for line in table.lines().skip(1) {
            let mut cells = line.split('\t');
            let (id, rarity) = (cells.next().unwrap_or(""), cells.next().unwrap_or(""));
            if rarity == "?" {
                continue;
            }
            let expected = !matches!(rarity, "Starter" | "Event" | "Ancient");
            assert_eq!(has_tradable_rarity(id), expected, "{id} ({rarity})");
        }
    }
}
