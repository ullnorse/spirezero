//! Regent's pools (`Models.CardPools/RegentCardPool.cs`, `Models.RelicPools/RegentRelicPool.cs`,
//! `Timeline.Epochs/Regent4Epoch.cs`) and starter deck (`Models.Characters/Regent.cs`).

use crate::content::cards::CardType::{Attack, Power, Skill};
use crate::run::cards::{PoolCard, pool_card};
use crate::run::odds::CardRarity::{Ancient, Basic, Common, Rare, Uncommon};
use crate::run::odds::PotionRarity;
use crate::run::relic_bag::RelicRarity;

/// `RegentCardPool.GenerateAllCards()` in declaration order. With every epoch revealed nothing is
/// filtered by epoch.
pub(super) const CARD_POOL: [PoolCard; 91] = [
    pool_card("ALIGNMENT", Uncommon, Skill, false),
    pool_card("ARSENAL", Rare, Power, false),
    pool_card("ASTRAL_PULSE", Common, Attack, false),
    pool_card("BEAT_INTO_SHAPE", Rare, Attack, false),
    pool_card("BEGONE", Common, Skill, false),
    pool_card("BIG_BANG", Rare, Skill, false),
    pool_card("BLACK_HOLE", Uncommon, Power, false),
    pool_card("BOMBARDMENT", Rare, Attack, false),
    pool_card("BULWARK", Uncommon, Skill, false),
    pool_card("BUNDLE_OF_JOY", Rare, Skill, false),
    pool_card("CELESTIAL_MIGHT", Common, Attack, false),
    pool_card("CHARGE", Uncommon, Skill, false),
    pool_card("CHILD_OF_THE_STARS", Uncommon, Power, false),
    pool_card("CLOAK_OF_STARS", Common, Skill, false),
    pool_card("COLLISION_COURSE", Common, Attack, false),
    pool_card("COMET", Rare, Attack, false),
    pool_card("CONQUEROR", Uncommon, Skill, false),
    pool_card("CONSTELLATION", Uncommon, Skill, true),
    pool_card("CONVERGENCE", Uncommon, Skill, false),
    pool_card("COSMIC_INDIFFERENCE", Common, Skill, false),
    pool_card("CRASH_LANDING", Rare, Attack, false),
    pool_card("CRESCENT_SPEAR", Common, Attack, false),
    pool_card("CRUSH_UNDER", Common, Attack, false),
    pool_card("DECISIONS_DECISIONS", Rare, Skill, false),
    pool_card("DEFEND_REGENT", Basic, Skill, false),
    pool_card("DEVASTATE", Uncommon, Attack, false),
    pool_card("DYING_STAR", Rare, Attack, false),
    pool_card("FALLING_STAR", Basic, Attack, false),
    pool_card("FOREGONE_CONCLUSION", Rare, Skill, false),
    pool_card("FURNACE", Uncommon, Power, false),
    pool_card("GAMMA_BLAST", Uncommon, Attack, false),
    pool_card("GATHER_LIGHT", Common, Skill, false),
    pool_card("GENESIS", Rare, Power, false),
    pool_card("GLIMMER", Uncommon, Skill, false),
    pool_card("GLITTERSTREAM", Common, Skill, false),
    pool_card("GLOW", Common, Skill, false),
    pool_card("GUARDS", Rare, Skill, false),
    pool_card("GUIDING_STAR", Common, Attack, false),
    pool_card("HAMMER_TIME", Rare, Power, true),
    pool_card("HEAVENLY_DRILL", Rare, Attack, false),
    pool_card("HEGEMONY", Uncommon, Attack, false),
    pool_card("HEIRLOOM_HAMMER", Rare, Attack, false),
    pool_card("HIDDEN_CACHE", Common, Skill, false),
    pool_card("I_AM_INVINCIBLE", Rare, Skill, false),
    pool_card("KINGLY_KICK", Uncommon, Attack, false),
    pool_card("KINGLY_PUNCH", Uncommon, Attack, false),
    pool_card("KNOCKOUT_BLOW", Uncommon, Attack, false),
    pool_card("KNOW_THY_PLACE", Common, Skill, false),
    pool_card("LARGESSE", Uncommon, Skill, true),
    pool_card("LUNAR_BLAST", Uncommon, Attack, false),
    pool_card("MAKE_IT_SO", Rare, Attack, false),
    pool_card("MANIFEST_AUTHORITY", Uncommon, Skill, false),
    pool_card("METEOR_SHOWER", Ancient, Attack, false),
    pool_card("MONARCHS_GAZE", Rare, Power, false),
    pool_card("MONOLOGUE", Uncommon, Skill, false),
    pool_card("NEUTRON_AEGIS", Rare, Power, false),
    pool_card("ORBIT", Uncommon, Power, false),
    pool_card("PALE_BLUE_DOT", Uncommon, Power, false),
    pool_card("PARRY", Uncommon, Power, false),
    pool_card("PARTICLE_WALL", Uncommon, Skill, false),
    pool_card("PATTER", Common, Skill, false),
    pool_card("PHOTON_CUT", Common, Attack, false),
    pool_card("PLOT", Uncommon, Skill, true),
    pool_card("PILLAR_OF_CREATION", Uncommon, Power, false),
    pool_card("PROPHESIZE", Uncommon, Skill, false),
    pool_card("QUASAR", Uncommon, Skill, false),
    pool_card("RADIATE", Uncommon, Attack, false),
    pool_card("REFINE_BLADE", Common, Skill, false),
    pool_card("REFLECT", Uncommon, Skill, false),
    pool_card("RESONANCE", Uncommon, Skill, false),
    pool_card("ROYAL_GAMBLE", Uncommon, Skill, false),
    pool_card("ROYALTIES", Rare, Power, false),
    pool_card("SEEKING_EDGE", Rare, Power, false),
    pool_card("SEVEN_STARS", Rare, Attack, false),
    pool_card("SHINING_STRIKE", Uncommon, Attack, false),
    pool_card("SOLAR_STRIKE", Common, Attack, false),
    pool_card("SPECTRUM_SHIFT", Uncommon, Power, false),
    pool_card("SPOILS_OF_BATTLE", Common, Skill, false),
    pool_card("STARDUST", Uncommon, Attack, false),
    pool_card("STRIKE_REGENT", Basic, Attack, false),
    pool_card("SUMMON_FORTH", Uncommon, Skill, false),
    pool_card("SUPERMASSIVE", Uncommon, Attack, false),
    pool_card("SWORD_SAGE", Rare, Power, false),
    pool_card("TERRAFORMING", Uncommon, Skill, false),
    pool_card("THE_SEALED_THRONE", Ancient, Power, false),
    pool_card("THE_SMITH", Rare, Skill, false),
    pool_card("TUTOR", Rare, Skill, true),
    pool_card("TYRANNY", Rare, Power, false),
    pool_card("VENERATE", Basic, Skill, false),
    pool_card("VOID_FORM", Rare, Power, false),
    pool_card("WROUGHT_IN_WAR", Common, Attack, false),
];

/// The pool cards whose `CanonicalKeywords` contain `Ethereal`.
pub(super) const ETHEREAL: [&str; 2] = ["DYING_STAR", "VOID_FORM"];

/// The pool cards that override `CanBeGeneratedInCombat` to false.
pub(super) const NOT_GENERATED: [&str; 1] = ["ROYALTIES"];

/// The pool cards with `CanonicalEnergyCost` 0 and no X cost.
pub(super) const ZERO_COST: [&str; 21] = [
    "ALIGNMENT",
    "ASTRAL_PULSE",
    "BIG_BANG",
    "CLOAK_OF_STARS",
    "COLLISION_COURSE",
    "COMET",
    "CONSTELLATION",
    "DECISIONS_DECISIONS",
    "FALLING_STAR",
    "GAMMA_BLAST",
    "KNOW_THY_PLACE",
    "LARGESSE",
    "LUNAR_BLAST",
    "MAKE_IT_SO",
    "METEOR_SHOWER",
    "MONOLOGUE",
    "PARTICLE_WALL",
    "QUASAR",
    "RADIATE",
    "ROYAL_GAMBLE",
    "STARDUST",
];

/// `RegentRelicPool.GenerateAllRelics()` in declaration order, with each relic's `Rarity`.
pub(super) const RELIC_POOL: [(&str, RelicRarity); 8] = [
    ("DIVINE_RIGHT", RelicRarity::Starter),
    ("FENCING_MANUAL", RelicRarity::Common),
    ("GALACTIC_DUST", RelicRarity::Uncommon),
    ("LUNAR_PASTRY", RelicRarity::Rare),
    ("MINI_REGENT", RelicRarity::Rare),
    ("ORANGE_DOUGH", RelicRarity::Rare),
    ("REGALITE", RelicRarity::Uncommon),
    ("VITRUVIAN_MINION", RelicRarity::Shop),
];

/// `RegentPotionPool` (`Regent4Epoch.Potions`), in declaration order, with each potion's `Rarity`.
pub(super) const POTION_POOL: [(&str, PotionRarity); 3] = [
    ("STAR_POTION", PotionRarity::Common),
    ("COSMIC_CONCOCTION", PotionRarity::Rare),
    ("KINGS_COURAGE", PotionRarity::Uncommon),
];

/// `Regent.StartingDeck`, in order.
pub(super) const STARTING_DECK: [&str; 10] = [
    "STRIKE_REGENT",
    "STRIKE_REGENT",
    "STRIKE_REGENT",
    "STRIKE_REGENT",
    "DEFEND_REGENT",
    "DEFEND_REGENT",
    "DEFEND_REGENT",
    "DEFEND_REGENT",
    "FALLING_STAR",
    "VENERATE",
];
