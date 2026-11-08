//! Defect's pools (`Models.CardPools/DefectCardPool.cs`, `Models.RelicPools/DefectRelicPool.cs`,
//! `Timeline.Epochs/Defect4Epoch.cs`) and starter deck (`Models.Characters/Defect.cs`).

use crate::content::cards::CardType::{Attack, Power, Skill};
use crate::run::cards::{PoolCard, pool_card};
use crate::run::odds::CardRarity::{Ancient, Basic, Common, Rare, Uncommon};
use crate::run::odds::PotionRarity;
use crate::run::relic_bag::RelicRarity;

/// `DefectCardPool.GenerateAllCards()` in declaration order. With every epoch revealed nothing is
/// filtered by epoch.
pub(super) const CARD_POOL: [PoolCard; 91] = [
    pool_card("ADAPTIVE_STRIKE", Rare, Attack, false),
    pool_card("ALL_FOR_ONE", Rare, Attack, false),
    pool_card("BALL_LIGHTNING", Common, Attack, false),
    pool_card("BARRAGE", Common, Attack, false),
    pool_card("BEAM_CELL", Common, Attack, false),
    pool_card("BIASED_COGNITION", Ancient, Power, false),
    pool_card("BOOST_AWAY", Common, Skill, false),
    pool_card("BOOT_SEQUENCE", Uncommon, Skill, false),
    pool_card("BUFFER", Rare, Power, false),
    pool_card("BULK_UP", Uncommon, Power, false),
    pool_card("CAPACITOR", Uncommon, Power, false),
    pool_card("CHAOS", Uncommon, Skill, false),
    pool_card("CHARGE_BATTERY", Common, Skill, false),
    pool_card("CHILL", Uncommon, Skill, false),
    pool_card("CLAW", Common, Attack, false),
    pool_card("COLD_SNAP", Common, Attack, false),
    pool_card("COMPACT", Uncommon, Skill, false),
    pool_card("COMPILE_DRIVER", Common, Attack, false),
    pool_card("CONSUMING_SHADOW", Rare, Power, false),
    pool_card("COOLANT", Rare, Power, false),
    pool_card("COOLHEADED", Common, Skill, false),
    pool_card("CREATIVE_AI", Rare, Power, false),
    pool_card("DARKNESS", Uncommon, Skill, false),
    pool_card("DEFEND_DEFECT", Basic, Skill, false),
    pool_card("DEFRAGMENT", Rare, Power, false),
    pool_card("DOUBLE_ENERGY", Uncommon, Skill, false),
    pool_card("DUALCAST", Basic, Skill, false),
    pool_card("ECHO_FORM", Rare, Power, false),
    pool_card("ENERGY_SURGE", Uncommon, Skill, true),
    pool_card("FERAL", Uncommon, Power, false),
    pool_card("FIGHT_THROUGH", Uncommon, Skill, false),
    pool_card("FLAK_CANNON", Rare, Attack, false),
    pool_card("FOCUSED_STRIKE", Common, Attack, false),
    pool_card("FTL", Uncommon, Attack, false),
    pool_card("FUSION", Uncommon, Skill, false),
    pool_card("GENETIC_ALGORITHM", Rare, Skill, false),
    pool_card("GLACIER", Uncommon, Skill, false),
    pool_card("GLASSWORK", Uncommon, Skill, false),
    pool_card("GO_FOR_THE_EYES", Common, Attack, false),
    pool_card("GUNK_UP", Common, Attack, false),
    pool_card("HAILSTORM", Uncommon, Power, false),
    pool_card("HELIX_DRILL", Rare, Attack, false),
    pool_card("HIBERNATE", Uncommon, Skill, true),
    pool_card("HOLOGRAM", Common, Skill, false),
    pool_card("HOTFIX", Common, Skill, false),
    pool_card("HYPERBEAM", Rare, Attack, false),
    pool_card("ICE_LANCE", Rare, Attack, false),
    pool_card("IGNITION", Uncommon, Skill, true),
    pool_card("IMITATION_LEARNING", Rare, Skill, true),
    pool_card("ITERATION", Uncommon, Power, false),
    pool_card("LEAP", Common, Skill, false),
    pool_card("LIGHTNING_ROD", Common, Skill, false),
    pool_card("LOOP", Uncommon, Power, false),
    pool_card("MACHINE_LEARNING", Rare, Power, false),
    pool_card("METEOR_STRIKE", Rare, Attack, false),
    pool_card("MODDED", Rare, Skill, false),
    pool_card("MOMENTUM_STRIKE", Common, Attack, false),
    pool_card("MULTI_CAST", Rare, Skill, false),
    pool_card("NULL", Uncommon, Attack, false),
    pool_card("ONE_FOR_ALL", Rare, Power, true),
    pool_card("OVERCLOCK", Uncommon, Skill, false),
    pool_card("QUADCAST", Ancient, Skill, false),
    pool_card("RAINBOW", Rare, Skill, false),
    pool_card("REBOOT", Rare, Skill, false),
    pool_card("REFRACT", Uncommon, Attack, false),
    pool_card("ROCKET_PUNCH", Uncommon, Attack, false),
    pool_card("SCAVENGE", Uncommon, Skill, false),
    pool_card("SCRAPE", Uncommon, Attack, false),
    pool_card("SHADOW_SHIELD", Uncommon, Skill, false),
    pool_card("SHATTER", Rare, Attack, false),
    pool_card("SIGNAL_BOOST", Rare, Skill, false),
    pool_card("SKIM", Uncommon, Skill, false),
    pool_card("SMOKESTACK", Uncommon, Power, false),
    pool_card("SPINNER", Rare, Power, false),
    pool_card("STORM", Uncommon, Power, false),
    pool_card("STRIKE_DEFECT", Basic, Attack, false),
    pool_card("SUBROUTINE", Uncommon, Power, false),
    pool_card("SUNDER", Uncommon, Attack, false),
    pool_card("SUPERCRITICAL", Rare, Skill, false),
    pool_card("SWEEPING_BEAM", Common, Attack, false),
    pool_card("SYNCHRONIZE", Uncommon, Skill, false),
    pool_card("SYNTHESIS", Uncommon, Attack, false),
    pool_card("TEMPEST", Uncommon, Skill, false),
    pool_card("TESLA_COIL", Uncommon, Attack, false),
    pool_card("THUNDER", Uncommon, Power, false),
    pool_card("TRASH_TO_TREASURE", Rare, Power, false),
    pool_card("TURBO", Common, Skill, false),
    pool_card("UPROAR", Common, Attack, false),
    pool_card("VOLTAIC", Rare, Skill, false),
    pool_card("WHITE_NOISE", Uncommon, Skill, false),
    pool_card("ZAP", Basic, Skill, false),
];

/// The pool cards whose `CanonicalKeywords` contain `Ethereal`.
pub(super) const ETHEREAL: [&str; 1] = ["ECHO_FORM"];

/// The pool cards that override `CanBeGeneratedInCombat` to false.
pub(super) const NOT_GENERATED: [&str; 0] = [];

/// The pool cards with `CanonicalEnergyCost` 0 and no X cost.
pub(super) const ZERO_COST: [&str; 15] = [
    "BEAM_CELL",
    "BOOST_AWAY",
    "BOOT_SEQUENCE",
    "CHILL",
    "CLAW",
    "FTL",
    "GO_FOR_THE_EYES",
    "HELIX_DRILL",
    "HOTFIX",
    "MODDED",
    "OVERCLOCK",
    "REBOOT",
    "SUPERCRITICAL",
    "TESLA_COIL",
    "TURBO",
];

/// `DefectRelicPool.GenerateAllRelics()` in declaration order, with each relic's `Rarity`.
pub(super) const RELIC_POOL: [(&str, RelicRarity); 8] = [
    ("CRACKED_CORE", RelicRarity::Starter),
    ("DATA_DISK", RelicRarity::Common),
    ("EMOTION_CHIP", RelicRarity::Rare),
    ("GOLD_PLATED_CABLES", RelicRarity::Uncommon),
    ("POWER_CELL", RelicRarity::Rare),
    ("METRONOME", RelicRarity::Rare),
    ("RUNIC_CAPACITOR", RelicRarity::Shop),
    ("SYMBIOTIC_VIRUS", RelicRarity::Uncommon),
];

/// `DefectPotionPool` (`Defect4Epoch.Potions`), in declaration order, with each potion's `Rarity`.
pub(super) const POTION_POOL: [(&str, PotionRarity); 3] = [
    ("FOCUS_POTION", PotionRarity::Common),
    ("ESSENCE_OF_DARKNESS", PotionRarity::Rare),
    ("POTION_OF_CAPACITY", PotionRarity::Uncommon),
];

/// `Defect.StartingDeck`, in order.
pub(super) const STARTING_DECK: [&str; 10] = [
    "STRIKE_DEFECT",
    "STRIKE_DEFECT",
    "STRIKE_DEFECT",
    "STRIKE_DEFECT",
    "DEFEND_DEFECT",
    "DEFEND_DEFECT",
    "DEFEND_DEFECT",
    "DEFEND_DEFECT",
    "ZAP",
    "DUALCAST",
];
