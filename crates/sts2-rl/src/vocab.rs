//! The id vocabularies the observation writes (`docs/reviews/spire-mentor.md`, idea 3).
//!
//! One append-only text file per content kind in `crates/sts2-rl/vocab/`, an id per line, the
//! line number being the id the observation uses. Line 0 is `<pad>` (an empty slot) and line 1
//! `<unk>` (content the file does not list yet), so a kind's id is never 0.
//!
//! The files only grow: new content is appended, so an id a trained net learned keeps its
//! meaning across builds. `vocab/pins.txt` records, per file, line counts with the hash of
//! that many first lines (one pin per append); the `vocab` test fails if a pinned prefix
//! changed, if a file has unpinned lines, or if the engine has a kind its file lacks (it
//! appends both with `STS2_VOCAB_APPEND=1`). [`hash`] covers every file and is part of the
//! FFI's build checks.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::OnceLock;

use sts2_core::combat::Encounter;
use sts2_core::content::ancients::AncientKind;
use sts2_core::content::cards::CardKind;
use sts2_core::content::events::EventKind;
use sts2_core::content::monsters::MonsterKind;
use sts2_core::content::potions::PotionKind;
use sts2_core::content::powers::PowerKind;
use sts2_core::content::relics::RelicKind;

/// The id of an empty slot.
pub const PAD: u16 = 0;
/// The id of content its file does not list.
pub const UNK: u16 = 1;

/// FNV-1a as a `Hasher`: the id tables hash short ASCII ids on every observation, where
/// `SipHash` costs more than the lookup.
#[derive(Clone, Copy, Debug)]
pub struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        Self(FNV_BASIS)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0 = fnv(self.0, bytes);
    }
}

/// A table of ids by name.
type Numbers = HashMap<&'static str, u16, BuildHasherDefault<FnvHasher>>;

/// A vocabulary: its file name and contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File {
    pub name: &'static str,
    pub text: &'static str,
}

/// Card kinds.
pub const CARDS: File = File {
    name: "cards.txt",
    text: include_str!("../vocab/cards.txt"),
};
/// Relic kinds.
pub const RELICS: File = File {
    name: "relics.txt",
    text: include_str!("../vocab/relics.txt"),
};
/// Monster ids.
pub const MONSTERS: File = File {
    name: "monsters.txt",
    text: include_str!("../vocab/monsters.txt"),
};
/// Power kinds.
pub const POWERS: File = File {
    name: "powers.txt",
    text: include_str!("../vocab/powers.txt"),
};
/// Potion kinds.
pub const POTIONS: File = File {
    name: "potions.txt",
    text: include_str!("../vocab/potions.txt"),
};
/// Encounters.
pub const ENCOUNTERS: File = File {
    name: "encounters.txt",
    text: include_str!("../vocab/encounters.txt"),
};

/// Events (the run observation's event page).
pub const EVENTS: File = File {
    name: "events.txt",
    text: include_str!("../vocab/events.txt"),
};
/// Ancients (the run observation's ancient page).
pub const ANCIENTS: File = File {
    name: "ancients.txt",
    text: include_str!("../vocab/ancients.txt"),
};
/// Acts (`ActInfo` ids).
pub const ACTS: File = File {
    name: "acts.txt",
    text: include_str!("../vocab/acts.txt"),
};

/// Every vocabulary, in [`hash`] order.
pub const FILES: [File; 9] = [
    CARDS, RELICS, MONSTERS, POWERS, POTIONS, ENCOUNTERS, EVENTS, ANCIENTS, ACTS,
];

impl File {
    /// The ids in id order.
    pub fn ids(self) -> impl Iterator<Item = &'static str> {
        self.text.lines()
    }

    /// Each listed id's number.
    fn numbers(self) -> Numbers {
        self.ids().zip(0..).skip(usize::from(UNK) + 1).collect()
    }

    /// The number of `id`, or [`UNK`].
    fn number(numbers: &Numbers, id: &str) -> u16 {
        numbers.get(id).copied().unwrap_or(UNK)
    }

    /// A table of each kind's id by discriminant.
    fn table<K: Copy>(
        self,
        kinds: impl Iterator<Item = K>,
        index: fn(K) -> usize,
        id: fn(K) -> &'static str,
    ) -> Vec<u16> {
        let numbers = self.numbers();
        let mut table = Vec::new();
        for kind in kinds {
            let i = index(kind);
            if table.len() <= i {
                table.resize(i + 1, UNK);
            }
            table[i] = Self::number(&numbers, id(kind));
        }
        table
    }
}

/// The FNV-1a hash of `bytes`, continuing from `h`.
#[must_use]
pub fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
    }
    h
}

/// The FNV-1a offset basis.
pub const FNV_BASIS: u64 = 0xCBF2_9CE4_8422_2325;

/// The hash of every vocabulary: FNV-1a over each file's name, a 0 byte, its contents and a
/// 0 byte, in [`FILES`] order.
#[must_use]
pub fn hash() -> u64 {
    FILES.iter().fold(FNV_BASIS, |h, f| {
        let h = fnv(fnv(h, f.name.as_bytes()), &[0]);
        fnv(fnv(h, f.text.as_bytes()), &[0])
    })
}

/// A lookup table, built on first use.
macro_rules! table {
    ($fn:ident, $kind:ty, $file:expr, $doc:literal) => {
        #[doc = $doc]
        #[must_use]
        pub fn $fn(kind: $kind) -> u16 {
            static TABLE: OnceLock<Vec<u16>> = OnceLock::new();
            TABLE.get_or_init(|| $file.table(<$kind>::ALL.into_iter(), |k| k as usize, <$kind>::id))
                [kind as usize]
        }
    };
}

table!(card, CardKind, CARDS, "A card kind's id.");
table!(relic, RelicKind, RELICS, "A relic kind's id.");
table!(power, PowerKind, POWERS, "A power kind's id.");
table!(potion, PotionKind, POTIONS, "A potion kind's id.");
table!(encounter, Encounter, ENCOUNTERS, "An encounter's id.");
table!(event, EventKind, EVENTS, "An event's id.");
table!(ancient, AncientKind, ANCIENTS, "An ancient's id.");

/// A monster's id, by [`MonsterKind::ordinal`].
#[must_use]
pub fn monster(kind: &MonsterKind) -> u16 {
    static TABLE: OnceLock<Vec<u16>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let numbers = MONSTERS.numbers();
        MonsterKind::IDS
            .iter()
            .map(|id| File::number(&numbers, id))
            .collect()
    });
    debug_assert_eq!(MonsterKind::IDS.get(kind.ordinal()), Some(&kind.id()));
    table.get(kind.ordinal()).copied().unwrap_or(UNK)
}

/// A lookup by content id string, built on first use.
macro_rules! by_id {
    ($fn:ident, $file:expr, $doc:literal) => {
        #[doc = $doc]
        #[must_use]
        pub fn $fn(id: &str) -> u16 {
            static NUMBERS: OnceLock<Numbers> = OnceLock::new();
            File::number(NUMBERS.get_or_init(|| $file.numbers()), id)
        }
    };
}

by_id!(
    card_id,
    CARDS,
    "A card's id, by its `ModelId.Entry` (run cards are strings)."
);
by_id!(
    monster_id,
    MONSTERS,
    "A monster's id, by its `ModelId.Entry`."
);
by_id!(relic_id, RELICS, "A relic's id, by its `ModelId.Entry`.");
by_id!(potion_id, POTIONS, "A potion's id, by its `ModelId.Entry`.");
by_id!(
    encounter_id,
    ENCOUNTERS,
    "An encounter's id, by its `ModelId.Entry`."
);
by_id!(event_id, EVENTS, "An event's id, by its `ModelId.Entry`.");
by_id!(act, ACTS, "An act's id, by its `ModelId.Entry`.");
