//! Necrobinder's pools (`Models.CardPools/NecrobinderCardPool.cs`, `Models.RelicPools/NecrobinderRelicPool.cs`,
//! `Timeline.Epochs/Necrobinder4Epoch.cs`) and starter deck (`Models.Characters/Necrobinder.cs`).

use crate::content::cards::CardType::{Attack, Power, Skill};
use crate::run::cards::{PoolCard, pool_card};
use crate::run::odds::CardRarity::{Ancient, Basic, Common, Rare, Uncommon};
use crate::run::odds::PotionRarity;
use crate::run::relic_bag::RelicRarity;

/// `NecrobinderCardPool.GenerateAllCards()` in declaration order. With every epoch revealed nothing is
/// filtered by epoch.
pub(super) const CARD_POOL: [PoolCard; 91] = [
    pool_card("AFTERLIFE", Common, Skill, false),
    pool_card("BANSHEES_CRY", Rare, Attack, false),
    pool_card("BLIGHT_STRIKE", Common, Attack, false),
    pool_card("BODYGUARD", Basic, Skill, false),
    pool_card("BONE_SHARDS", Uncommon, Attack, false),
    pool_card("BORROWED_TIME", Uncommon, Skill, false),
    pool_card("BURY", Uncommon, Attack, false),
    pool_card("CACOPHONY", Rare, Power, true),
    pool_card("CALCIFY", Uncommon, Power, false),
    pool_card("CALL_OF_THE_VOID", Rare, Power, false),
    pool_card("CAPTURE_SPIRIT", Uncommon, Skill, false),
    pool_card("CLEANSE", Uncommon, Skill, false),
    pool_card("COUNTDOWN", Uncommon, Power, false),
    pool_card("DANSE_MACABRE", Uncommon, Power, false),
    pool_card("DEATH_MARCH", Uncommon, Attack, false),
    pool_card("DEATHBRINGER", Uncommon, Skill, false),
    pool_card("DEATHS_DOOR", Uncommon, Skill, false),
    pool_card("DEBILITATE", Uncommon, Attack, false),
    pool_card("DEFEND_NECROBINDER", Basic, Skill, false),
    pool_card("DEFILE", Common, Attack, false),
    pool_card("DEFY", Common, Skill, false),
    pool_card("DELAY", Uncommon, Skill, false),
    pool_card("DEMESNE", Rare, Power, false),
    pool_card("DEVOUR_LIFE", Rare, Power, false),
    pool_card("DIRGE", Uncommon, Skill, false),
    pool_card("DRAIN_POWER", Common, Attack, false),
    pool_card("DREDGE", Uncommon, Skill, false),
    pool_card("EIDOLON", Rare, Skill, false),
    pool_card("END_OF_DAYS", Rare, Skill, false),
    pool_card("ENFEEBLING_TOUCH", Uncommon, Skill, false),
    pool_card("ERADICATE", Rare, Attack, false),
    pool_card("FEAR", Common, Attack, false),
    pool_card("FETCH", Uncommon, Attack, false),
    pool_card("FLATTEN", Common, Attack, false),
    pool_card("FORBIDDEN_GRIMOIRE", Ancient, Power, false),
    pool_card("FRIENDSHIP", Uncommon, Power, false),
    pool_card("GLIMPSE_BEYOND", Rare, Skill, true),
    pool_card("GRAVE_WARDEN", Common, Skill, false),
    pool_card("GRAVEBLAST", Common, Attack, false),
    pool_card("HANG", Rare, Attack, false),
    pool_card("HAUNT", Uncommon, Power, false),
    pool_card("HIGH_FIVE", Uncommon, Attack, false),
    pool_card("INVOKE", Common, Skill, false),
    pool_card("LEGION_OF_BONE", Uncommon, Skill, true),
    pool_card("LETHALITY", Uncommon, Power, false),
    pool_card("MELANCHOLY", Uncommon, Skill, false),
    pool_card("MISERY", Rare, Attack, false),
    pool_card("NECRO_MASTERY", Rare, Power, false),
    pool_card("NEGATIVE_PULSE", Common, Skill, false),
    pool_card("NEUROSURGE", Rare, Power, false),
    pool_card("NO_ESCAPE", Uncommon, Skill, false),
    pool_card("OBLIVION", Rare, Skill, false),
    pool_card("PAGESTORM", Uncommon, Power, false),
    pool_card("PARSE", Uncommon, Skill, false),
    pool_card("POKE", Common, Attack, false),
    pool_card("PROTECTOR", Ancient, Attack, false),
    pool_card("PULL_AGGRO", Common, Skill, false),
    pool_card("PULL_FROM_BELOW", Uncommon, Attack, false),
    pool_card("PUTREFY", Uncommon, Skill, false),
    pool_card("RATTLE", Uncommon, Attack, false),
    pool_card("REANIMATE", Rare, Skill, false),
    pool_card("REAP", Common, Attack, false),
    pool_card("REAPER_FORM", Rare, Power, false),
    pool_card("REAVE", Common, Attack, false),
    pool_card("RIGHT_HAND_HAND", Uncommon, Attack, false),
    pool_card("SACRIFICE", Rare, Skill, false),
    pool_card("SCOURGE", Common, Skill, false),
    pool_card("SCULPTING_STRIKE", Common, Attack, false),
    pool_card("SEANCE", Rare, Skill, false),
    pool_card("SENTRY_MODE", Rare, Power, false),
    pool_card("SEVERANCE", Uncommon, Attack, false),
    pool_card("SHARED_FATE", Rare, Skill, false),
    pool_card("SHROUD", Uncommon, Power, false),
    pool_card("SIC_EM", Uncommon, Attack, false),
    pool_card("SLEIGHT_OF_FLESH", Uncommon, Power, false),
    pool_card("SNAP", Common, Attack, false),
    pool_card("SOUL_STORM", Rare, Attack, false),
    pool_card("SOULBOUND", Uncommon, Power, true),
    pool_card("SOW", Common, Attack, false),
    pool_card("SPIRIT_OF_ASH", Rare, Power, false),
    pool_card("SPUR", Uncommon, Skill, false),
    pool_card("SQUEEZE", Rare, Attack, false),
    pool_card("STRIKE_NECROBINDER", Basic, Attack, false),
    pool_card("THE_SCYTHE", Rare, Attack, false),
    pool_card("TIMES_UP", Rare, Attack, false),
    pool_card("TRANSFIGURE", Rare, Skill, false),
    pool_card("UNDEATH", Rare, Skill, false),
    pool_card("UNDERWORLD", Uncommon, Skill, true),
    pool_card("UNLEASH", Basic, Attack, false),
    pool_card("VEILPIERCER", Uncommon, Attack, false),
    pool_card("WISP", Common, Skill, false),
];

/// The pool cards whose `CanonicalKeywords` contain `Ethereal`.
pub(super) const ETHEREAL: [&str; 8] = [
    "DEFILE",
    "DEFY",
    "DEMESNE",
    "ENFEEBLING_TOUCH",
    "FEAR",
    "LETHALITY",
    "PARSE",
    "SEANCE",
];

/// The pool cards that override `CanBeGeneratedInCombat` to false.
pub(super) const NOT_GENERATED: [&str; 2] = ["EIDOLON", "TRANSFIGURE"];

/// The pool cards with `CanonicalEnergyCost` 0 and no X cost.
pub(super) const ZERO_COST: [&str; 9] = [
    "FETCH",
    "MISERY",
    "NEUROSURGE",
    "OBLIVION",
    "POKE",
    "RIGHT_HAND_HAND",
    "SHARED_FATE",
    "UNDEATH",
    "WISP",
];

/// `NecrobinderRelicPool.GenerateAllRelics()` in declaration order, with each relic's `Rarity`.
pub(super) const RELIC_POOL: [(&str, RelicRarity); 8] = [
    ("BIG_HAT", RelicRarity::Rare),
    ("BONE_FLUTE", RelicRarity::Common),
    ("BOOK_REPAIR_KNIFE", RelicRarity::Uncommon),
    ("BOOKMARK", RelicRarity::Rare),
    ("BOUND_PHYLACTERY", RelicRarity::Starter),
    ("FUNERARY_MASK", RelicRarity::Uncommon),
    ("IVORY_TILE", RelicRarity::Rare),
    ("UNDYING_SIGIL", RelicRarity::Shop),
];

/// `NecrobinderPotionPool` (`Necrobinder4Epoch.Potions`), in declaration order, with each potion's `Rarity`.
pub(super) const POTION_POOL: [(&str, PotionRarity); 3] = [
    ("POTION_OF_DOOM", PotionRarity::Common),
    ("POT_OF_GHOULS", PotionRarity::Rare),
    ("BONE_BREW", PotionRarity::Uncommon),
];

/// `Necrobinder.StartingDeck`, in order.
pub(super) const STARTING_DECK: [&str; 10] = [
    "STRIKE_NECROBINDER",
    "STRIKE_NECROBINDER",
    "STRIKE_NECROBINDER",
    "STRIKE_NECROBINDER",
    "DEFEND_NECROBINDER",
    "DEFEND_NECROBINDER",
    "DEFEND_NECROBINDER",
    "DEFEND_NECROBINDER",
    "BODYGUARD",
    "UNLEASH",
];
