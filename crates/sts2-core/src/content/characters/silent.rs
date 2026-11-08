//! Silent's pools (`Models.CardPools/SilentCardPool.cs`, `Models.RelicPools/SilentRelicPool.cs`,
//! `Timeline.Epochs/Silent4Epoch.cs`) and starter deck (`Models.Characters/Silent.cs`).

use crate::content::cards::CardType::{Attack, Power, Skill};
use crate::run::cards::{PoolCard, pool_card};
use crate::run::odds::CardRarity::{Ancient, Basic, Common, Rare, Uncommon};
use crate::run::odds::PotionRarity;
use crate::run::relic_bag::RelicRarity;

/// `SilentCardPool.GenerateAllCards()` in declaration order. With every epoch revealed nothing is
/// filtered by epoch.
pub(super) const CARD_POOL: [PoolCard; 91] = [
    pool_card("ABRASIVE", Rare, Power, false),
    pool_card("ACCELERANT", Uncommon, Power, false),
    pool_card("ACCURACY", Uncommon, Power, false),
    pool_card("ACROBATICS", Uncommon, Skill, false),
    pool_card("ADRENALINE", Rare, Skill, false),
    pool_card("AFTERIMAGE", Rare, Power, false),
    pool_card("ANTICIPATE", Common, Skill, false),
    pool_card("ASSASSINATE", Rare, Attack, false),
    pool_card("BACKFLIP", Common, Skill, false),
    pool_card("BACKSTAB", Uncommon, Attack, false),
    pool_card("BLADE_OF_INK", Rare, Skill, false),
    pool_card("BLADE_DANCE", Common, Skill, false),
    pool_card("BLADE_SYMPHONY", Uncommon, Skill, true),
    pool_card("BLUR", Uncommon, Skill, false),
    pool_card("BOUNCING_FLASK", Uncommon, Skill, false),
    pool_card("BUBBLE_BUBBLE", Uncommon, Skill, false),
    pool_card("BULLET_TIME", Rare, Skill, false),
    pool_card("BURST", Rare, Skill, false),
    pool_card("CALCULATED_GAMBLE", Uncommon, Skill, false),
    pool_card("CLOAK_AND_DAGGER", Common, Skill, false),
    pool_card("CORROSIVE_WAVE", Rare, Skill, false),
    pool_card("CONCOCT", Uncommon, Skill, true),
    pool_card("DAGGER_SPRAY", Common, Attack, false),
    pool_card("DAGGER_THROW", Common, Attack, false),
    pool_card("DASH", Uncommon, Attack, false),
    pool_card("DEADLY_POISON", Common, Skill, false),
    pool_card("DEFEND_SILENT", Basic, Skill, false),
    pool_card("DEFLECT", Common, Skill, false),
    pool_card("DODGE_AND_ROLL", Common, Skill, false),
    pool_card("ECHOING_SLASH", Uncommon, Attack, false),
    pool_card("ENVENOM", Rare, Power, false),
    pool_card("ESCAPE_PLAN", Uncommon, Skill, false),
    pool_card("EXPERTISE", Uncommon, Skill, false),
    pool_card("EXPOSE", Uncommon, Skill, false),
    pool_card("FADE", Uncommon, Skill, true),
    pool_card("FAN_OF_KNIVES", Rare, Power, false),
    pool_card("FINISHER", Uncommon, Attack, false),
    pool_card("FLANKING", Rare, Skill, true),
    pool_card("FLECHETTES", Uncommon, Attack, false),
    pool_card("FLICK_FLACK", Common, Attack, false),
    pool_card("SIDESTEP", Uncommon, Skill, false),
    pool_card("FOOTWORK", Uncommon, Power, false),
    pool_card("GRAND_FINALE", Rare, Attack, false),
    pool_card("HAND_TRICK", Uncommon, Skill, false),
    pool_card("HAZE", Uncommon, Skill, false),
    pool_card("HIDDEN_DAGGERS", Uncommon, Skill, false),
    pool_card("INFINITE_BLADES", Uncommon, Power, false),
    pool_card("KNIFE_TRAP", Rare, Skill, false),
    pool_card("LEADING_STRIKE", Common, Attack, false),
    pool_card("LEG_SWEEP", Uncommon, Skill, false),
    pool_card("MALAISE", Rare, Skill, false),
    pool_card("MASTER_PLANNER", Rare, Power, false),
    pool_card("MEMENTO_MORI", Uncommon, Attack, false),
    pool_card("MIRAGE", Uncommon, Skill, false),
    pool_card("MURDER", Rare, Attack, false),
    pool_card("NEUTRALIZE", Basic, Attack, false),
    pool_card("NIGHTMARE", Rare, Skill, false),
    pool_card("NOXIOUS_FUMES", Uncommon, Power, false),
    pool_card("OUTBREAK", Rare, Skill, false),
    pool_card("PHANTOM_BLADES", Uncommon, Power, false),
    pool_card("PIERCING_WAIL", Common, Skill, false),
    pool_card("PINPOINT", Uncommon, Attack, false),
    pool_card("POISONED_STAB", Common, Attack, false),
    pool_card("POUNCE", Uncommon, Attack, false),
    pool_card("PRECISE_CUT", Uncommon, Attack, false),
    pool_card("PREDATOR", Common, Attack, false),
    pool_card("PREPARED", Common, Skill, false),
    pool_card("REFLEX", Uncommon, Skill, false),
    pool_card("RICOCHET", Common, Attack, false),
    pool_card("SERPENT_FORM", Rare, Power, false),
    pool_card("SHADOW_STEP", Rare, Skill, false),
    pool_card("SHADOWMELD", Rare, Skill, false),
    pool_card("SKEWER", Uncommon, Attack, false),
    pool_card("SLICE", Common, Attack, false),
    pool_card("SNAKEBITE", Common, Skill, false),
    pool_card("SNEAKY", Rare, Power, true),
    pool_card("SPEEDSTER", Uncommon, Power, false),
    pool_card("STORM_OF_STEEL", Rare, Skill, false),
    pool_card("STRANGLE", Uncommon, Attack, false),
    pool_card("STRIKE_SILENT", Basic, Attack, false),
    pool_card("SUCKER_PUNCH", Common, Attack, false),
    pool_card("SUPPRESS", Ancient, Attack, false),
    pool_card("SURVIVOR", Basic, Skill, false),
    pool_card("TACTICIAN", Uncommon, Skill, false),
    pool_card("THE_HUNT", Rare, Attack, false),
    pool_card("TOOLS_OF_THE_TRADE", Rare, Power, false),
    pool_card("TRACKING", Rare, Power, false),
    pool_card("UNTOUCHABLE", Common, Skill, false),
    pool_card("UP_MY_SLEEVE", Uncommon, Skill, false),
    pool_card("WELL_LAID_PLANS", Rare, Power, false),
    pool_card("WRAITH_FORM", Ancient, Power, false),
];

/// The pool cards whose `CanonicalKeywords` contain `Ethereal`.
pub(super) const ETHEREAL: [&str; 0] = [];

/// The pool cards that override `CanBeGeneratedInCombat` to false.
pub(super) const NOT_GENERATED: [&str; 2] = ["NIGHTMARE", "THE_HUNT"];

/// The pool cards with `CanonicalEnergyCost` 0 and no X cost.
pub(super) const ZERO_COST: [&str; 18] = [
    "ADRENALINE",
    "ANTICIPATE",
    "ASSASSINATE",
    "BACKSTAB",
    "CALCULATED_GAMBLE",
    "CONCOCT",
    "DEFLECT",
    "ESCAPE_PLAN",
    "EXPOSE",
    "FADE",
    "SIDESTEP",
    "GRAND_FINALE",
    "HIDDEN_DAGGERS",
    "NEUTRALIZE",
    "PRECISE_CUT",
    "PREPARED",
    "SLICE",
    "SUPPRESS",
];

/// `SilentRelicPool.GenerateAllRelics()` in declaration order, with each relic's `Rarity`.
pub(super) const RELIC_POOL: [(&str, RelicRarity); 8] = [
    ("HELICAL_DART", RelicRarity::Rare),
    ("NINJA_SCROLL", RelicRarity::Shop),
    ("PAPER_KRANE", RelicRarity::Rare),
    ("RING_OF_THE_SNAKE", RelicRarity::Starter),
    ("SNECKO_SKULL", RelicRarity::Common),
    ("TINGSHA", RelicRarity::Uncommon),
    ("TOUGH_BANDAGES", RelicRarity::Rare),
    ("TWISTED_FUNNEL", RelicRarity::Uncommon),
];

/// `SilentPotionPool` (`Silent4Epoch.Potions`), in declaration order, with each potion's `Rarity`.
pub(super) const POTION_POOL: [(&str, PotionRarity); 3] = [
    ("POISON_POTION", PotionRarity::Common),
    ("GHOST_IN_A_JAR", PotionRarity::Rare),
    ("CUNNING_POTION", PotionRarity::Uncommon),
];

/// `Silent.StartingDeck`, in order.
pub(super) const STARTING_DECK: [&str; 12] = [
    "STRIKE_SILENT",
    "STRIKE_SILENT",
    "STRIKE_SILENT",
    "STRIKE_SILENT",
    "STRIKE_SILENT",
    "DEFEND_SILENT",
    "DEFEND_SILENT",
    "DEFEND_SILENT",
    "DEFEND_SILENT",
    "DEFEND_SILENT",
    "NEUTRALIZE",
    "SURVIVOR",
];
