//! Acts and their pre-rolled rooms: `ActModel.GenerateRooms` and `RoomSet`.
//!
//! Every act's rooms are rolled at run start on `UpFront`, so all three acts of the run (Act 1 is
//! Overgrowth or Underdocks, see [`select_acts`]; then Hive and Glory) are listed here even though
//! only Act 1 is played.
//!
//! The act tables are complete room-generation data (ids, room types, `IsWeak`, `Tags`), ported or
//! not: room generation draws over the whole list. Porting an encounter or event therefore changes
//! nothing here; only a new event-only encounter needs a line in `EVENT_ENCOUNTERS`.

use super::ascension::AscensionLevel;
#[cfg(test)]
use super::unlock::IdSet;
use super::unlock::UnlockState;
use crate::rng::{GrabBag, Rng};
use crate::run::RoomType;

/// `RoomType` of an encounter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EncounterRoom {
    /// A monster encounter.
    Monster,
    /// An elite encounter.
    Elite,
    /// A boss encounter.
    Boss,
}

/// An encounter as the act lists see it: id, room type, `IsWeak` and `Tags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EncounterInfo {
    /// `ModelId.Entry`.
    pub id: &'static str,
    /// The room type.
    pub room: EncounterRoom,
    /// `IsWeak`.
    pub is_weak: bool,
    /// `Tags`.
    pub tags: &'static [&'static str],
}

impl EncounterInfo {
    /// `SharesTagsWith`: any tag in common.
    fn shares_tags_with(&self, other: &Self) -> bool {
        self.tags.iter().any(|t| other.tags.contains(t))
    }
}

const fn enc(
    id: &'static str,
    room: EncounterRoom,
    is_weak: bool,
    tags: &'static [&'static str],
) -> EncounterInfo {
    EncounterInfo {
        id,
        room,
        is_weak,
        tags,
    }
}

use EncounterRoom::{Boss, Elite, Monster};

/// Encounters in no act's list that an event starts (`EnterCombatWithoutExitingEvent`), with their
/// `EncounterModel.RoomType`.
const EVENT_ENCOUNTERS: &[(&str, RoomType)] = &[
    ("DENSE_VEGETATION_EVENT_ENCOUNTER", RoomType::Monster),
    ("MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER", RoomType::Monster),
    ("PUNCH_OFF_EVENT_ENCOUNTER", RoomType::Monster),
    // `RoomType` from `Models.Encounters/<Name>.cs:9-11`. Hive's `TheLanternKey`:
    ("MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER", RoomType::Monster),
    // Glory's `BattlewornDummy` (the base `BattlewornDummyEventEncounter` is abstract):
    ("BATTLEWORN_DUMMY_EVENT_V1_ENCOUNTER", RoomType::Monster),
    ("BATTLEWORN_DUMMY_EVENT_V2_ENCOUNTER", RoomType::Monster),
    ("BATTLEWORN_DUMMY_EVENT_V3_ENCOUNTER", RoomType::Monster),
    // The shared `FakeMerchant`:
    ("FAKE_MERCHANT_EVENT_ENCOUNTER", RoomType::Monster),
];

/// `EncounterModel.RoomType` of the encounter with `ModelId.Entry` `entry`, from every act's
/// encounter list ([`ALL_ACTS`]) and the events' own encounters; `None` for an encounter in none
/// of them.
#[must_use]
pub fn room_type_of_encounter(entry: &str) -> Option<RoomType> {
    ALL_ACTS
        .iter()
        .flat_map(|act| act.encounters)
        .find(|e| e.id == entry)
        .map(|e| match e.room {
            Monster => RoomType::Monster,
            Elite => RoomType::Elite,
            Boss => RoomType::Boss,
        })
        .or_else(|| {
            EVENT_ENCOUNTERS
                .iter()
                .find(|&&(id, _)| id == entry)
                .map(|&(_, room)| room)
        })
}

/// An act model's room-generation data (`Models.Acts/*.cs`).
#[derive(Debug)]
pub struct ActInfo {
    /// `ModelId.Entry`.
    pub id: &'static str,
    /// `AllEvents`, in declaration order.
    pub events: &'static [&'static str],
    /// `GenerateAllEncounters()`, in declaration order.
    pub encounters: &'static [EncounterInfo],
    /// `AllAncients` (`GetUnlockedAncients` drops nothing with every epoch revealed).
    pub ancients: &'static [&'static str],
    /// `NumberOfWeakEncounters`.
    pub weak_encounters: usize,
    /// `BaseNumberOfRooms` (single player: `GetNumberOfRooms`).
    pub rooms: usize,
}

/// `Overgrowth` (`Models.Acts/Overgrowth.cs`).
pub const OVERGROWTH: ActInfo = ActInfo {
    id: "OVERGROWTH",
    events: &[
        "AROMA_OF_CHAOS",
        "BYRDONIS_NEST",
        "DENSE_VEGETATION",
        "JUNGLE_MAZE_ADVENTURE",
        "LUMINOUS_CHOIR",
        "MORPHIC_GROVE",
        "SAPPHIRE_SEED",
        "SUNKEN_STATUE",
        "TABLET_OF_TRUTH",
        "UNREST_SITE",
        "WELLSPRING",
        "WHISPERING_HOLLOW",
        "WOOD_CARVINGS",
    ],
    encounters: &[
        enc("BYGONE_EFFIGY_ELITE", Elite, false, &[]),
        enc("BYRDONIS_ELITE", Elite, false, &[]),
        enc("CEREMONIAL_BEAST_BOSS", Boss, false, &[]),
        enc("CUBEX_CONSTRUCT_NORMAL", Monster, false, &[]),
        enc("FLYCONID_NORMAL", Monster, false, &["Mushroom", "Slimes"]),
        enc("FOGMOG_NORMAL", Monster, false, &[]),
        enc("FUZZY_WURM_CRAWLER_WEAK", Monster, true, &["Crawler"]),
        enc("INKLETS_NORMAL", Monster, false, &[]),
        enc("MAWLER_NORMAL", Monster, false, &[]),
        enc("NIBBITS_NORMAL", Monster, false, &[]),
        enc("NIBBITS_WEAK", Monster, true, &["Nibbit"]),
        enc(
            "OVERGROWTH_CRAWLERS",
            Monster,
            false,
            &["Shrinker", "Crawler"],
        ),
        enc("PHROG_PARASITE_ELITE", Elite, false, &[]),
        enc("RUBY_RAIDERS_NORMAL", Monster, false, &[]),
        enc("SHRINKER_BEETLE_WEAK", Monster, true, &["Shrinker"]),
        enc("SLIMES_NORMAL", Monster, false, &["Slimes"]),
        enc("SLIMES_WEAK", Monster, true, &["Slimes"]),
        enc(
            "SLITHERING_STRANGLER_NORMAL",
            Monster,
            false,
            &["Jaxfruit", "Slimes"],
        ),
        enc(
            "SNAPPING_JAXFRUIT_NORMAL",
            Monster,
            false,
            &["Mushroom", "Jaxfruit"],
        ),
        enc("THE_KIN_BOSS", Boss, false, &[]),
        enc("VANTOM_BOSS", Boss, false, &[]),
        enc("VINE_SHAMBLER_NORMAL", Monster, false, &[]),
    ],
    ancients: &["NEOW"],
    weak_encounters: 3,
    rooms: 15,
};

/// `Underdocks` (`Models.Acts/Underdocks.cs`), the alternate Act 1.
pub const UNDERDOCKS: ActInfo = ActInfo {
    id: "UNDERDOCKS",
    events: &[
        "ABYSSAL_BATHS",
        "DROWNING_BEACON",
        "ENDLESS_CONVEYOR",
        "PUNCH_OFF",
        "SPIRALING_WHIRLPOOL",
        "SUNKEN_STATUE",
        "SUNKEN_TREASURY",
        "DOORS_OF_LIGHT_AND_DARK",
        "TRASH_HEAP",
        "WATERLOGGED_SCRIPTORIUM",
    ],
    encounters: &[
        enc("CORPSE_SLUGS_NORMAL", Monster, false, &["Slugs"]),
        enc("CORPSE_SLUGS_WEAK", Monster, true, &["Slugs"]),
        enc("CULTISTS_NORMAL", Monster, false, &[]),
        enc("FOSSIL_STALKER_NORMAL", Monster, false, &[]),
        enc("GREMLIN_MERC_NORMAL", Monster, false, &[]),
        enc("HAUNTED_SHIP_NORMAL", Monster, false, &[]),
        enc("LAGAVULIN_MATRIARCH_BOSS", Boss, false, &[]),
        enc("LIVING_FOG_NORMAL", Monster, false, &[]),
        enc("PHANTASMAL_GARDENERS_ELITE", Elite, false, &[]),
        enc("PUNCH_CONSTRUCT_NORMAL", Monster, false, &[]),
        enc("SEAPUNK_NORMAL", Monster, false, &["Seapunk"]),
        enc("SEAPUNK_WEAK", Monster, true, &["Seapunk"]),
        enc("SEWER_CLAM_NORMAL", Monster, false, &[]),
        enc("SKULKING_COLONY_ELITE", Elite, false, &[]),
        enc("SLUDGE_SPINNER_WEAK", Monster, true, &[]),
        enc("SOUL_FYSH_BOSS", Boss, false, &[]),
        enc("TERROR_EEL_ELITE", Elite, false, &[]),
        enc("TOADPOLES_WEAK", Monster, true, &[]),
        enc("TWO_TAILED_RATS_NORMAL", Monster, false, &[]),
        enc("WATERFALL_GIANT_BOSS", Boss, false, &[]),
    ],
    ancients: &["NEOW"],
    weak_encounters: 3,
    rooms: 15,
};

/// `Hive` (`Models.Acts/Hive.cs`).
pub const HIVE: ActInfo = ActInfo {
    id: "HIVE",
    events: &[
        "AMALGAMATOR",
        "BUGSLAYER",
        "COLORFUL_PHILOSOPHERS",
        "COLOSSAL_FLOWER",
        "FIELD_OF_MAN_SIZED_HOLES",
        "INFESTED_AUTOMATON",
        "LOST_WISP",
        "SPIRIT_GRAFTER",
        "THE_LANTERN_KEY",
        "ZEN_WEAVER",
    ],
    encounters: &[
        enc("BOWLBUGS_NORMAL", Monster, false, &["Workers"]),
        enc("BOWLBUGS_WEAK", Monster, true, &["Workers"]),
        enc("CHOMPERS_NORMAL", Monster, false, &["Chomper"]),
        enc("DECIMILLIPEDE_ELITE", Elite, false, &[]),
        enc("ENTOMANCER_ELITE", Elite, false, &[]),
        enc("EXOSKELETONS_NORMAL", Monster, false, &["Exoskeletons"]),
        enc("EXOSKELETONS_WEAK", Monster, true, &["Exoskeletons"]),
        enc("HUNTER_KILLER_NORMAL", Monster, false, &[]),
        enc("KAISER_CRAB_BOSS", Boss, false, &[]),
        enc("INFESTED_PRISMS_ELITE", Elite, false, &[]),
        enc("KNOWLEDGE_DEMON_BOSS", Boss, false, &[]),
        enc("LOUSE_PROGENITOR_NORMAL", Monster, false, &[]),
        enc("MYTES_NORMAL", Monster, false, &[]),
        enc("OVICOPTER_NORMAL", Monster, false, &[]),
        enc("SLUMBERING_BEETLE_NORMAL", Monster, false, &["Workers"]),
        enc("SPINY_TOAD_NORMAL", Monster, false, &[]),
        enc("THE_INSATIABLE_BOSS", Boss, false, &[]),
        enc("THE_OBSCURA_NORMAL", Monster, false, &[]),
        enc("THIEVING_HOPPER_WEAK", Monster, true, &["Thieves"]),
        enc("TUNNELER_WEAK", Monster, true, &["Burrower"]),
    ],
    ancients: &["OROBAS", "PAEL", "TEZCATARA"],
    weak_encounters: 2,
    rooms: 14,
};

/// `Glory` (`Models.Acts/Glory.cs`).
pub const GLORY: ActInfo = ActInfo {
    id: "GLORY",
    events: &[
        "BATTLEWORN_DUMMY",
        "GRAVE_OF_THE_FORGOTTEN",
        "HUNGRY_FOR_MUSHROOMS",
        "REFLECTIONS",
        "ROUND_TEA_PARTY",
        "TRIAL",
        "TINKER_TIME",
    ],
    encounters: &[
        enc("AXEBOTS_NORMAL", Monster, false, &[]),
        enc("CONSTRUCT_MENAGERIE_NORMAL", Monster, false, &[]),
        enc("DEVOTED_SCULPTOR_WEAK", Monster, true, &[]),
        enc("AEONGLASS_BOSS", Boss, false, &[]),
        enc("FABRICATOR_NORMAL", Monster, false, &[]),
        enc("FROG_KNIGHT_NORMAL", Monster, false, &[]),
        enc("GLOBE_HEAD_NORMAL", Monster, false, &[]),
        enc("KNIGHTS_ELITE", Elite, false, &["Knights"]),
        enc("MECHA_KNIGHT_ELITE", Elite, false, &[]),
        enc("OWL_MAGISTRATE_NORMAL", Monster, false, &[]),
        enc("QUEEN_BOSS", Boss, false, &[]),
        enc("SCROLLS_OF_BITING_NORMAL", Monster, false, &["Scrolls"]),
        enc("SCROLLS_OF_BITING_WEAK", Monster, true, &["Scrolls"]),
        enc("SLIMED_BERSERKER_NORMAL", Monster, false, &[]),
        enc("SOUL_NEXUS_ELITE", Elite, false, &[]),
        enc("TEST_SUBJECT_BOSS", Boss, false, &[]),
        enc("THE_LOST_AND_FORGOTTEN_NORMAL", Monster, false, &[]),
        enc("TURRET_OPERATOR_WEAK", Monster, true, &[]),
    ],
    ancients: &["NONUPEIPE", "TANX", "VAKUU"],
    weak_encounters: 2,
    rooms: 13,
};

/// `ActModel.GetDefaultList()`.
pub const DEFAULT_ACTS: [&ActInfo; 3] = [&OVERGROWTH, &HIVE, &GLORY];

/// `ModelDb.ActsByIndex` (`ModelDb.cs:323-345`): `ModelDb.Acts` bucketed by `Index`, in
/// `ModelDb.Acts` order (Overgrowth, Underdocks, Hive, Glory).
pub const ACTS_BY_INDEX: [&[&ActInfo]; 3] = [&[&OVERGROWTH, &UNDERDOCKS], &[&HIVE], &[&GLORY]];

/// `ModelDb.Acts`: every act.
pub const ALL_ACTS: [&ActInfo; 4] = [&OVERGROWTH, &UNDERDOCKS, &HIVE, &GLORY];

/// The lobby's `Act1` setting (`StartRunLobby.Act1`, default `"random"`): which act is Act 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Act1 {
    /// `"random"`: the `act_selection` roll decides.
    #[default]
    Random,
    /// `"overgrowth"`.
    Overgrowth,
    /// `"underdocks"`.
    Underdocks,
}

impl Act1 {
    /// The lobby key (`"random"`, `"overgrowth"`, `"underdocks"`), also the harness's `ACT1`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Overgrowth => "overgrowth",
            Self::Underdocks => "underdocks",
        }
    }

    /// The setting with lobby key `key`; `None` for an unknown key (the game then keeps the roll,
    /// which [`Act1::Random`] spells explicitly).
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        [Self::Random, Self::Overgrowth, Self::Underdocks]
            .into_iter()
            .find(|a| a.key() == key)
    }

    /// `StartRunLobby.GetAct`: the forced act, if any.
    const fn forced(self) -> Option<&'static ActInfo> {
        match self {
            Self::Random => None,
            Self::Overgrowth => Some(&OVERGROWTH),
            Self::Underdocks => Some(&UNDERDOCKS),
        }
    }
}

/// `ActModel.IsUnlocked`: Underdocks needs `UnderdocksEpoch` (`Underdocks.cs:107-110`); the others
/// are always unlocked.
fn is_unlocked(act: &ActInfo, unlock: &UnlockState) -> bool {
    act.id != UNDERDOCKS.id || unlock.epochs_revealed.contains("UNDERDOCKS_EPOCH")
}

/// The run's acts as `StartRunLobby.BeginRunLocally` picks them (`StartRunLobby.cs:470-473`).
///
/// `ActModel.GetRandomList` (`ActModel.cs:538-565`) on `new Rng(hash(seed), "act_selection")`, a
/// stream of its own, then `list[0] = GetAct(Act1) ?? list[0]`.
///
/// Per act index, `NextItem` of the unlocked acts, drawn even for one candidate. The profile is an
/// experienced one (`docs/specs/test-mode.md`): every act is discovered, so the first-sighting
/// shortcut (an undiscovered non-default act returned with no draw, test mode off) never applies.
/// With Underdocks unlocked, Act 1 is `NextInt(0, 2)`: 0 Overgrowth, 1 Underdocks. The draw is made
/// even when `act1` forces the act.
///
/// # Panics
/// Never: every index has an unlocked act.
#[must_use]
pub fn select_acts(seed: u64, unlock: &UnlockState, act1: Act1) -> [&'static ActInfo; 3] {
    let mut rng = Rng::named(seed, "act_selection");
    let mut picked = ACTS_BY_INDEX.map(|candidates| {
        let unlocked: Vec<&'static ActInfo> = candidates
            .iter()
            .copied()
            .filter(|act| is_unlocked(act, unlock))
            .collect();
        *rng.next_item(&unlocked)
            .expect("every act index has an unlocked act")
    });
    if let Some(act) = act1.forced() {
        picked[0] = act;
    }
    picked
}

/// `ModelDb.AllSharedEvents`, in declaration order.
pub const SHARED_EVENTS: [&str; 18] = [
    "BRAIN_LEECH",
    "CRYSTAL_SPHERE",
    "DOLL_ROOM",
    "FAKE_MERCHANT",
    "POTION_COURIER",
    "RANWID_THE_ELDER",
    "RELIC_TRADER",
    "ROOM_FULL_OF_CHEESE",
    "SELF_HELP_BOOK",
    "SLIPPERY_BRIDGE",
    "STONE_OF_ALL_TIME",
    "SYMBIOTE",
    "TEA_MASTER",
    "THE_FUTURE_OF_POTIONS",
    "THE_LEGENDS_WERE_TRUE",
    "THIS_OR_THAT",
    "WAR_HISTORIAN_REPY",
    "WELCOME_TO_WONGOS",
];

/// `ModelDb.AllSharedAncients` (`UnlockState.SharedAncients` with `DarvEpoch` revealed).
pub const SHARED_ANCIENTS: [&str; 1] = ["DARV"];

/// The number of elite encounters rolled per act (`ActModel.GenerateRooms`).
const ELITES_PER_ACT: usize = 15;

/// One act's pre-rolled rooms and visit counters (`Rooms/RoomSet.cs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSet {
    /// The act.
    pub act: &'static ActInfo,
    /// `Events`.
    pub events: Vec<&'static str>,
    /// Events visited.
    pub events_visited: usize,
    /// `NormalEncounters`.
    pub normal_encounters: Vec<&'static EncounterInfo>,
    /// Normal encounters visited.
    pub normal_encounters_visited: usize,
    /// `EliteEncounters`.
    pub elite_encounters: Vec<&'static EncounterInfo>,
    /// Elite encounters visited.
    pub elite_encounters_visited: usize,
    /// Boss encounters visited.
    pub boss_encounters_visited: usize,
    /// `Boss`.
    pub boss: &'static EncounterInfo,
    /// `SecondBoss`: the last act's from `DoubleBoss` on.
    pub second_boss: Option<&'static EncounterInfo>,
    /// The act's ancient (`ModelId.Entry`).
    pub ancient: &'static str,
}

impl PartialEq for ActInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ActInfo {}

impl RoomSet {
    /// `NextNormalEncounter`: `normalEncounters[visited % count]`.
    #[must_use]
    pub fn next_normal_encounter(&self) -> &'static EncounterInfo {
        self.normal_encounters[self.normal_encounters_visited % self.normal_encounters.len()]
    }

    /// `NextEliteEncounter`.
    #[must_use]
    pub fn next_elite_encounter(&self) -> &'static EncounterInfo {
        self.elite_encounters[self.elite_encounters_visited % self.elite_encounters.len()]
    }

    /// `NextBossEncounter`: the second boss once a boss of this act was visited, else `Boss`.
    #[must_use]
    pub const fn next_boss_encounter(&self) -> &'static EncounterInfo {
        match self.second_boss {
            Some(second) if self.boss_encounters_visited != 0 => second,
            _ => self.boss,
        }
    }
}

/// `RunManager.GenerateRooms` (`RunManager.cs:743-767`) for the run's `acts`, on `UpFront`.
///
/// Shuffles the shared ancients, gives acts 2 and 3 a `NextInt(n + 1)`-sized prefix of them,
/// then runs `ActModel.GenerateRooms` per act. Tutorial order is off. From `DoubleBoss` on, the
/// last act then draws its second boss: `NextItem` of its bosses other than `Boss`.
///
/// # Panics
/// Never: the act tables are fixed and every act has a boss and an ancient; `acts` is not empty.
pub fn generate_rooms(acts: &[&'static ActInfo], rng: &mut Rng, ascension: u8) -> Vec<RoomSet> {
    let mut shared_ancients = SHARED_ANCIENTS.to_vec();
    crate::shuffle::unstable_shuffle(&mut shared_ancients, rng);
    let mut subsets = vec![Vec::new()];
    for _ in acts.iter().skip(1) {
        let bound = i32::try_from(shared_ancients.len() + 1).expect("few shared ancients");
        let count = usize::try_from(rng.next_int(bound)).expect("next_int is non-negative");
        let subset: Vec<&str> = shared_ancients.drain(..count).collect();
        subsets.push(subset);
    }
    let mut rooms: Vec<RoomSet> = acts
        .iter()
        .zip(subsets)
        .map(|(act, subset)| generate_act_rooms(act, &subset, rng))
        .collect();
    if AscensionLevel::DoubleBoss.is_active(ascension) {
        let last = rooms.last_mut().expect("a run has acts");
        let others: Vec<&'static EncounterInfo> = last
            .act
            .encounters
            .iter()
            .filter(|e| e.room == Boss && e.id != last.boss.id)
            .collect();
        last.second_boss = rng.next_item(&others).copied();
    }
    rooms
}

/// `ActModel.GenerateRooms` (`ActModel.cs:331-386`) with every epoch revealed.
fn generate_act_rooms(
    act: &'static ActInfo,
    shared_ancients: &[&'static str],
    rng: &mut Rng,
) -> RoomSet {
    let mut events: Vec<&'static str> = act.events.iter().chain(&SHARED_EVENTS).copied().collect();
    crate::shuffle::unstable_shuffle(&mut events, rng);

    let weak: Vec<&'static EncounterInfo> = act
        .encounters
        .iter()
        .filter(|e| e.room == Monster && e.is_weak)
        .collect();
    let regular: Vec<&'static EncounterInfo> = act
        .encounters
        .iter()
        .filter(|e| e.room == Monster && !e.is_weak)
        .collect();
    let elites: Vec<&'static EncounterInfo> =
        act.encounters.iter().filter(|e| e.room == Elite).collect();
    let bosses: Vec<&'static EncounterInfo> =
        act.encounters.iter().filter(|e| e.room == Boss).collect();

    let mut normal_encounters = Vec::new();
    fill_from_bag(&mut normal_encounters, &weak, act.weak_encounters, rng);
    fill_from_bag(
        &mut normal_encounters,
        &regular,
        act.rooms - act.weak_encounters,
        rng,
    );
    let mut elite_encounters = Vec::new();
    fill_from_bag(&mut elite_encounters, &elites, ELITES_PER_ACT, rng);

    let boss = *rng.next_item(&bosses).expect("every act has a boss");
    let ancients: Vec<&'static str> = act
        .ancients
        .iter()
        .chain(shared_ancients)
        .copied()
        .collect();
    let ancient = *rng.next_item(&ancients).expect("every act has an ancient");
    RoomSet {
        act,
        events,
        events_visited: 0,
        normal_encounters,
        normal_encounters_visited: 0,
        elite_encounters,
        elite_encounters_visited: 0,
        boss_encounters_visited: 0,
        boss,
        second_boss: None,
        ancient,
    }
}

/// `count` picks of `AddWithoutRepeatingTags` from a weight-1 `GrabBag` of `pool`, refilled
/// whenever it runs empty.
fn fill_from_bag(
    list: &mut Vec<&'static EncounterInfo>,
    pool: &[&'static EncounterInfo],
    count: usize,
    rng: &mut Rng,
) {
    let mut bag = GrabBag::new();
    for _ in 0..count {
        if bag.is_empty() {
            for &e in pool {
                bag.add(e, 1.0);
            }
        }
        add_without_repeating_tags(list, &mut bag, rng);
    }
}

/// `ActModel.AddWithoutRepeatingTags` (`ActModel.cs:419-430`): grab an encounter that shares
/// no tag with the previous one and is not it; if none qualifies, grab any.
fn add_without_repeating_tags(
    list: &mut Vec<&'static EncounterInfo>,
    bag: &mut GrabBag<&'static EncounterInfo>,
    rng: &mut Rng,
) {
    let previous = list.last().copied();
    let picked = bag
        .grab_and_remove(rng, |e| {
            previous.is_none_or(|p| !e.shares_tags_with(p) && e.id != p.id)
        })
        .or_else(|| bag.grab_and_remove(rng, |_| true));
    if let Some(e) = picked {
        list.push(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{RunRngSet, canonicalize_seed};
    use std::collections::BTreeSet;

    fn seed(s: &str) -> u64 {
        RunRngSet::new(&canonicalize_seed(s)).seed()
    }

    /// Seeds of the player's own runs and the Act 1 they recorded (`tools/replay_history.py`
    /// checks all of them).
    const RECORDED: [(&str, &str); 4] = [
        ("UAW89J6J159S", "OVERGROWTH"),
        ("YTVGLP85FEJG", "OVERGROWTH"),
        ("931669RW3SD9", "UNDERDOCKS"),
        ("UZSYUPM16B95", "UNDERDOCKS"),
    ];

    #[test]
    fn random_act1_is_the_first_act_selection_draw() {
        for (s, recorded) in RECORDED {
            let acts = select_acts(seed(s), &UnlockState::all(), Act1::Random);
            assert_eq!(acts[0].id, recorded, "{s}");
            // `GetRandomList`: `NextItem` of [Overgrowth, Underdocks] is the stream's first draw.
            let roll = Rng::named(seed(s), "act_selection").next_int(2);
            assert_eq!(acts[0], ACTS_BY_INDEX[0][usize::try_from(roll).unwrap()]);
            assert_eq!(acts[1..], [&HIVE, &GLORY]);
        }
    }

    #[test]
    fn act1_setting_overrides_the_roll() {
        for (s, _) in RECORDED {
            let s = seed(s);
            let unlock = UnlockState::all();
            assert_eq!(select_acts(s, &unlock, Act1::Overgrowth), DEFAULT_ACTS);
            assert_eq!(
                select_acts(s, &unlock, Act1::Underdocks),
                [&UNDERDOCKS, &HIVE, &GLORY]
            );
        }
    }

    #[test]
    fn underdocks_needs_its_epoch() {
        let unlock = UnlockState {
            epochs_revealed: IdSet::Only(BTreeSet::new()),
            ..UnlockState::all()
        };
        for (s, _) in RECORDED {
            assert_eq!(select_acts(seed(s), &unlock, Act1::Random), DEFAULT_ACTS);
        }
    }

    #[test]
    fn act1_keys_round_trip() {
        for a in [Act1::Random, Act1::Overgrowth, Act1::Underdocks] {
            assert_eq!(Act1::from_key(a.key()), Some(a));
        }
        assert_eq!(Act1::from_key("hive"), None);
    }

    #[test]
    fn underdocks_rooms_come_from_its_lists() {
        let mut rng = Rng::new(7);
        let rooms = generate_rooms(&[&UNDERDOCKS, &HIVE, &GLORY], &mut rng, 0);
        let ud = &rooms[0];
        assert_eq!(ud.act, &UNDERDOCKS);
        assert_eq!(
            ud.events.len(),
            UNDERDOCKS.events.len() + SHARED_EVENTS.len()
        );
        assert_eq!(ud.normal_encounters.len(), 15);
        assert!(ud.normal_encounters[..3].iter().all(|e| e.is_weak));
        assert!(
            ud.normal_encounters
                .iter()
                .all(|e| UNDERDOCKS.encounters.contains(e))
        );
        assert_eq!(ud.boss.room, Boss);
        assert_eq!(ud.ancient, "NEOW");
        assert_eq!(
            room_type_of_encounter("TERROR_EEL_ELITE"),
            Some(RoomType::Elite)
        );
        assert_eq!(
            room_type_of_encounter("PUNCH_OFF_EVENT_ENCOUNTER"),
            Some(RoomType::Monster)
        );
    }

    /// Later acts' encounters, list and event-only (`Models.Encounters/*.cs` `RoomType`).
    #[test]
    fn room_types_of_later_acts_encounters() {
        assert_eq!(
            room_type_of_encounter("KAISER_CRAB_BOSS"),
            Some(RoomType::Boss)
        );
        assert_eq!(
            room_type_of_encounter("SOUL_NEXUS_ELITE"),
            Some(RoomType::Elite)
        );
        for id in [
            "MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER",
            "BATTLEWORN_DUMMY_EVENT_V1_ENCOUNTER",
            "BATTLEWORN_DUMMY_EVENT_V2_ENCOUNTER",
            "BATTLEWORN_DUMMY_EVENT_V3_ENCOUNTER",
            "FAKE_MERCHANT_EVENT_ENCOUNTER",
        ] {
            assert_eq!(room_type_of_encounter(id), Some(RoomType::Monster), "{id}");
        }
        assert_eq!(
            room_type_of_encounter("BATTLEWORN_DUMMY_EVENT_ENCOUNTER"),
            None
        );
    }
}
