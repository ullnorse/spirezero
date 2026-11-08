//! `RunRngSet` and `PlayerRngSet`: the named, persisted RNG streams of a run and of each player.

use std::ops::{Index, IndexMut};

use super::{Rng, snake_case, xxh64};

/// A run-scoped stream (`RunRngType`), in the game's enum order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunRngType {
    /// `RunRngType.UpFront`.
    UpFront,
    /// `RunRngType.Shuffle`.
    Shuffle,
    /// `RunRngType.UnknownMapPoint`.
    UnknownMapPoint,
    /// `RunRngType.CombatCardGeneration`.
    CombatCardGeneration,
    /// `RunRngType.CombatPotionGeneration`.
    CombatPotionGeneration,
    /// `RunRngType.CombatCardSelection`.
    CombatCardSelection,
    /// `RunRngType.CombatEnergyCosts`.
    CombatEnergyCosts,
    /// `RunRngType.CombatTargets`.
    CombatTargets,
    /// `RunRngType.MonsterAi`.
    MonsterAi,
    /// `RunRngType.Niche`.
    Niche,
    /// `RunRngType.CombatOrbs`.
    CombatOrbs,
    /// `RunRngType.TreasureRoomRelics`.
    TreasureRoomRelics,
}

impl_index!(RunRngType, PlayerRngType);

impl RunRngType {
    /// Every stream, in enum order.
    pub const ALL: [Self; 12] = [
        Self::UpFront,
        Self::Shuffle,
        Self::UnknownMapPoint,
        Self::CombatCardGeneration,
        Self::CombatPotionGeneration,
        Self::CombatCardSelection,
        Self::CombatEnergyCosts,
        Self::CombatTargets,
        Self::MonsterAi,
        Self::Niche,
        Self::CombatOrbs,
        Self::TreasureRoomRelics,
    ];

    /// The C# enum member name (`ToString()`), which is what the stream name is derived from.
    #[must_use]
    pub const fn enum_name(self) -> &'static str {
        match self {
            Self::UpFront => "UpFront",
            Self::Shuffle => "Shuffle",
            Self::UnknownMapPoint => "UnknownMapPoint",
            Self::CombatCardGeneration => "CombatCardGeneration",
            Self::CombatPotionGeneration => "CombatPotionGeneration",
            Self::CombatCardSelection => "CombatCardSelection",
            Self::CombatEnergyCosts => "CombatEnergyCosts",
            Self::CombatTargets => "CombatTargets",
            Self::MonsterAi => "MonsterAi",
            Self::Niche => "Niche",
            Self::CombatOrbs => "CombatOrbs",
            Self::TreasureRoomRelics => "TreasureRoomRelics",
        }
    }

    /// The hashed stream name: `snake_case` of the enum member name.
    #[must_use]
    pub fn stream_name(self) -> String {
        snake_case(self.enum_name())
    }
}

/// A per-player stream (`PlayerRngType`), in the game's enum order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerRngType {
    /// `PlayerRngType.Rewards`.
    Rewards,
    /// `PlayerRngType.Shops`.
    Shops,
    /// `PlayerRngType.Transformations`.
    Transformations,
}

impl PlayerRngType {
    /// Every stream, in enum order.
    pub const ALL: [Self; 3] = [Self::Rewards, Self::Shops, Self::Transformations];

    /// The C# enum member name (`ToString()`).
    #[must_use]
    pub const fn enum_name(self) -> &'static str {
        match self {
            Self::Rewards => "Rewards",
            Self::Shops => "Shops",
            Self::Transformations => "Transformations",
        }
    }

    /// The hashed stream name: `snake_case` of the enum member name.
    #[must_use]
    pub fn stream_name(self) -> String {
        snake_case(self.enum_name())
    }
}

/// The run's streams, each seeded with `Rng::named(seed, stream_name)`.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RunRngSet {
    seed_string: String,
    seed: u64,
    rngs: [Rng; RunRngType::ALL.len()],
}

impl_clone_by_fields!(RunRngSet {
    seed_string,
    seed,
    rngs,
});

impl RunRngSet {
    /// `new RunRngSet(seed)`: `seed = xxh64(seed_string)`, then one stream per type.
    ///
    /// # Panics
    /// If `seed_string` starts with `"old"`: pre-v19 saves use a legacy hash that is not
    /// ported (canonical seeds are upper-case, so this never happens for new runs).
    #[must_use]
    pub fn new(seed_string: &str) -> Self {
        assert!(
            !seed_string.starts_with("old"),
            "RunRngSet: legacy \"old\" seeds are not supported"
        );
        let seed = xxh64(seed_string);
        Self {
            seed_string: seed_string.to_owned(),
            seed,
            rngs: RunRngType::ALL.map(|kind| Rng::named(seed, &kind.stream_name())),
        }
    }

    /// An empty set left behind when the streams are moved to their owner of the moment (a
    /// combat, `docs/decisions.md`); never drawn from.
    pub(crate) fn placeholder() -> Self {
        Self {
            seed_string: String::new(),
            seed: 0,
            rngs: RunRngType::ALL.map(|_| Rng::new(0)),
        }
    }

    /// A set from its parts, as [`crate::text`] reads it back.
    pub(crate) const fn from_parts(
        seed_string: String,
        seed: u64,
        rngs: [Rng; RunRngType::ALL.len()],
    ) -> Self {
        Self {
            seed_string,
            seed,
            rngs,
        }
    }

    /// The seed string the run was started with (saved as `seed`).
    #[must_use]
    pub fn seed_string(&self) -> &str {
        &self.seed_string
    }

    /// The hashed run seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }
}

impl Index<RunRngType> for RunRngSet {
    type Output = Rng;

    fn index(&self, kind: RunRngType) -> &Rng {
        &self.rngs[kind.index()]
    }
}

impl IndexMut<RunRngType> for RunRngSet {
    fn index_mut(&mut self, kind: RunRngType) -> &mut Rng {
        &mut self.rngs[kind.index()]
    }
}

/// One player's streams, each seeded with `Rng::named(player_seed, stream_name)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayerRngSet {
    seed: u64,
    rngs: [Rng; PlayerRngType::ALL.len()],
}

impl PlayerRngSet {
    /// `new PlayerRngSet(seed)`. The game passes `xxh64(seed_string) + slot_index`; see
    /// [`PlayerRngSet::for_slot`].
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rngs: PlayerRngType::ALL.map(|kind| Rng::named(seed, &kind.stream_name())),
        }
    }

    /// The set for the player in `slot_index`: `new(run_seed + slot_index)`.
    #[must_use]
    pub fn for_slot(run_seed: u64, slot_index: u64) -> Self {
        Self::new(run_seed.wrapping_add(slot_index))
    }

    /// A set from its parts, as [`crate::text`] reads it back.
    pub(crate) const fn from_parts(seed: u64, rngs: [Rng; PlayerRngType::ALL.len()]) -> Self {
        Self { seed, rngs }
    }

    /// The player seed the streams derive from.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }
}

impl Index<PlayerRngType> for PlayerRngSet {
    type Output = Rng;

    fn index(&self, kind: PlayerRngType) -> &Rng {
        &self.rngs[kind.index()]
    }
}

impl IndexMut<PlayerRngType> for PlayerRngSet {
    fn index_mut(&mut self, kind: PlayerRngType) -> &mut Rng {
        &mut self.rngs[kind.index()]
    }
}
