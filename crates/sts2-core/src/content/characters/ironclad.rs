//! Ironclad's pools (`Models.CardPools/IroncladCardPool.cs`, `Models.RelicPools/IroncladRelicPool.cs`,
//! `Timeline.Epochs/Ironclad4Epoch.cs`) and starter deck (`Models.Characters/Ironclad.cs`).

use crate::content::cards::CardType::{Attack, Power, Skill};
use crate::run::cards::{PoolCard, pool_card};
use crate::run::odds::CardRarity::{Ancient, Basic, Common, Rare, Uncommon};
use crate::run::odds::PotionRarity;
use crate::run::relic_bag::RelicRarity;

/// `IroncladCardPool.GenerateAllCards()` in declaration order. With every epoch revealed nothing is
/// filtered by epoch.
pub(super) const CARD_POOL: [PoolCard; 90] = [
    pool_card("AGGRESSION", Rare, Power, false),
    pool_card("ANGER", Common, Attack, false),
    pool_card("ARMAMENTS", Common, Skill, false),
    pool_card("ASHEN_STRIKE", Uncommon, Attack, false),
    pool_card("BARRICADE", Rare, Power, false),
    pool_card("BASH", Basic, Attack, false),
    pool_card("BATTLE_TRANCE", Uncommon, Skill, false),
    pool_card("BLAZE", Uncommon, Skill, true),
    pool_card("BLOOD_WALL", Common, Skill, false),
    pool_card("BLOODLETTING", Uncommon, Skill, false),
    pool_card("BLUDGEON", Uncommon, Attack, false),
    pool_card("BODY_SLAM", Common, Attack, false),
    pool_card("BRAND", Rare, Skill, false),
    pool_card("BREAK", Ancient, Attack, false),
    pool_card("BREAKTHROUGH", Common, Attack, false),
    pool_card("BULLY", Uncommon, Attack, false),
    pool_card("BURNING_PACT", Uncommon, Skill, false),
    pool_card("CASCADE", Rare, Skill, false),
    pool_card("CINDER", Common, Attack, false),
    pool_card("COLOSSUS", Uncommon, Skill, false),
    pool_card("CONFLAGRATION", Rare, Attack, false),
    pool_card("CORRUPTION", Ancient, Power, false),
    pool_card("CRIMSON_MANTLE", Rare, Power, false),
    pool_card("CRUELTY", Uncommon, Power, false),
    pool_card("DARK_EMBRACE", Rare, Power, false),
    pool_card("DEFEND_IRONCLAD", Basic, Skill, false),
    pool_card("DEMON_FORM", Rare, Power, false),
    pool_card("DEMONIC_SHIELD", Uncommon, Skill, true),
    pool_card("DISMANTLE", Uncommon, Attack, false),
    pool_card("DOMINATE", Rare, Skill, false),
    pool_card("DRUM_OF_BATTLE", Uncommon, Skill, false),
    pool_card("EVIL_EYE", Uncommon, Skill, false),
    pool_card("EXPECT_A_FIGHT", Uncommon, Skill, false),
    pool_card("FEED", Rare, Attack, false),
    pool_card("FEEL_NO_PAIN", Uncommon, Power, false),
    pool_card("FIEND_FIRE", Rare, Attack, false),
    pool_card("FIGHT_ME", Uncommon, Attack, false),
    pool_card("FLAME_BARRIER", Uncommon, Skill, false),
    pool_card("FORGOTTEN_RITUAL", Uncommon, Skill, false),
    pool_card("HAVOC", Common, Skill, false),
    pool_card("HEADBUTT", Common, Attack, false),
    pool_card("HELLRAISER", Rare, Power, false),
    pool_card("HEMOKINESIS", Uncommon, Attack, false),
    pool_card("HOWL_FROM_BEYOND", Uncommon, Attack, false),
    pool_card("IMPERVIOUS", Rare, Skill, false),
    pool_card("INFERNAL_BLADE", Uncommon, Skill, false),
    pool_card("INFERNO", Uncommon, Power, false),
    pool_card("INFLAME", Uncommon, Power, false),
    pool_card("IRON_WAVE", Common, Attack, false),
    pool_card("JUGGERNAUT", Rare, Power, false),
    pool_card("JUGGLING", Uncommon, Power, false),
    pool_card("MANGLE", Rare, Attack, false),
    pool_card("MIDNIGHT", Rare, Attack, true),
    pool_card("MOLTEN_FIST", Common, Attack, false),
    pool_card("NOT_YET", Rare, Skill, false),
    pool_card("OFFERING", Rare, Skill, false),
    pool_card("ONE_TWO_PUNCH", Rare, Skill, false),
    pool_card("OUTRAGE", Uncommon, Attack, true),
    pool_card("PACTS_END", Rare, Attack, false),
    pool_card("PERFECTED_STRIKE", Common, Attack, false),
    pool_card("PILLAGE", Uncommon, Attack, false),
    pool_card("POMMEL_STRIKE", Common, Attack, false),
    pool_card("PRIMAL_FORCE", Rare, Skill, false),
    pool_card("PYRE", Rare, Power, false),
    pool_card("RAGE", Uncommon, Skill, false),
    pool_card("RAMPAGE", Uncommon, Attack, false),
    pool_card("RUPTURE", Uncommon, Power, false),
    pool_card("SECOND_WIND", Uncommon, Skill, false),
    pool_card("SETUP_STRIKE", Common, Attack, false),
    pool_card("SHRUG_IT_OFF", Common, Skill, false),
    pool_card("SPITE", Uncommon, Attack, false),
    pool_card("STAMPEDE", Uncommon, Power, false),
    pool_card("STOKE", Rare, Skill, false),
    pool_card("STOMP", Uncommon, Attack, false),
    pool_card("STONE_ARMOR", Uncommon, Power, false),
    pool_card("STRIKE_IRONCLAD", Basic, Attack, false),
    pool_card("SWORD_BOOMERANG", Common, Attack, false),
    pool_card("TANK", Rare, Power, true),
    pool_card("TAUNT", Common, Skill, false),
    pool_card("TEAR_ASUNDER", Rare, Attack, false),
    pool_card("THRASH", Rare, Attack, false),
    pool_card("THUNDERCLAP", Common, Attack, false),
    pool_card("TREMBLE", Common, Skill, false),
    pool_card("TRUE_GRIT", Common, Skill, false),
    pool_card("TWIN_STRIKE", Common, Attack, false),
    pool_card("UNMOVABLE", Rare, Power, false),
    pool_card("UNRELENTING", Uncommon, Attack, false),
    pool_card("UPPERCUT", Uncommon, Attack, false),
    pool_card("VICIOUS", Uncommon, Power, false),
    pool_card("WHIRLWIND", Uncommon, Attack, false),
];

/// The pool cards whose `CanonicalKeywords` contain `Ethereal`.
pub(super) const ETHEREAL: [&str; 0] = [];

/// The pool cards that override `CanBeGeneratedInCombat` to false.
pub(super) const NOT_GENERATED: [&str; 2] = ["FEED", "NOT_YET"];

/// The pool cards with `CanonicalEnergyCost` 0 and no X cost.
pub(super) const ZERO_COST: [&str; 12] = [
    "ANGER",
    "BATTLE_TRANCE",
    "BLOODLETTING",
    "BRAND",
    "BULLY",
    "DEMONIC_SHIELD",
    "OFFERING",
    "OUTRAGE",
    "PACTS_END",
    "PRIMAL_FORCE",
    "RAGE",
    "SPITE",
];

/// `IroncladRelicPool.GenerateAllRelics()` in declaration order, with each relic's `Rarity`.
pub(super) const RELIC_POOL: [(&str, RelicRarity); 8] = [
    ("BRIMSTONE", RelicRarity::Shop),
    ("BURNING_BLOOD", RelicRarity::Starter),
    ("CHARONS_ASHES", RelicRarity::Rare),
    ("DEMON_TONGUE", RelicRarity::Rare),
    ("PAPER_PHROG", RelicRarity::Uncommon),
    ("RED_SKULL", RelicRarity::Common),
    ("RUINED_HELMET", RelicRarity::Rare),
    ("SELF_FORMING_CLAY", RelicRarity::Uncommon),
];

/// `IroncladPotionPool` (`Ironclad4Epoch.Potions`), in declaration order, with each potion's `Rarity`.
pub(super) const POTION_POOL: [(&str, PotionRarity); 3] = [
    ("BLOOD_POTION", PotionRarity::Common),
    ("SOLDIERS_STEW", PotionRarity::Rare),
    ("ASHWATER", PotionRarity::Uncommon),
];

/// `Ironclad.StartingDeck`, in order.
pub(super) const STARTING_DECK: [&str; 10] = [
    "STRIKE_IRONCLAD",
    "STRIKE_IRONCLAD",
    "STRIKE_IRONCLAD",
    "STRIKE_IRONCLAD",
    "STRIKE_IRONCLAD",
    "DEFEND_IRONCLAD",
    "DEFEND_IRONCLAD",
    "DEFEND_IRONCLAD",
    "DEFEND_IRONCLAD",
    "BASH",
];
