//! The run observation's event section (RLN33, `RunOptions::obs_events`): what each event or
//! ancient option's text tells a player, as numbers.
//!
//! Written at [`EVENTS_AT`] (after the full-view section, which does not move) only with
//! `obs_events`, while an event page or an ancient's page is pending; zero otherwise.
//! [`OPTIONS`] × [`OPTION_BYTES`], in the option order of the event or ancient section:
//! [`COLUMNS`] `i16` numbers ([`COLUMN_NAMES`]), then [`ID_SLOTS`] `u16` vocab ids
//! ([`ID_SLOT_NAMES`]: a card, a curse, a relic, a potion it gives, and a card, relic and potion
//! it takes; 0 none), then the enchantment it applies (`u8`, `EnchantmentKind` index + 1, 0
//! none) and 0.
//!
//! **What the numbers are.** Each option's effects are written down by hand from its text
//! ([`EVENT_SPECS`], by text key; [`RELIC_SPECS`], a relic's pickup effects, for ancient
//! options and the events whose options are relics), as a short spec: `hp=-$HpLoss` is "lose
//! `{HpLoss}` HP", `$Name` a `DynamicVar` of the event as its text shows it now
//! ([`EventRoom::vars`]: the rolled and set values, never the event's `Rng` or the run's
//! streams) or, for a relic, its canonical var. `known` is 1 when every effect the text states
//! is in the vector and every var it needs has a value; an option with no spec, or one whose
//! text needs a column the vector lacks, has `known` 0 and whatever it could fill.
//!
//! The columns are signed amounts (`hp` +heal/-loss, `max_hp`, `gold` +gain/-pay), counts
//! (cards added, curses added, card rewards, choices offered, cards removed, upgraded,
//! downgraded, transformed and enchanted, relics gained and lost, potions gained and lost,
//! potion slots), the enchantment's amount, the rarity of a random relic (1 common), the HP of a
//! fight's foe, the combats a reward waits for, the minigame's turns, and flags: heals to full,
//! loses all gold, applies to all matching cards, duplicates the deck, starts a fight, the
//! outcome is random (a random card, relic, potion or card to change), the player chooses the
//! cards, another page follows, the option only leaves.

use std::collections::HashMap;
use std::sync::OnceLock;

use sts2_core::content::ancients::AncientOption;
use sts2_core::content::cards::{CardKind, CardType};
use sts2_core::content::enchantments::EnchantmentKind;
use sts2_core::content::events::{EventData, EventKind, EventOption, EventVar};
use sts2_core::content::potions::PotionKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::content::tables::VarText;
use sts2_core::run::events::EventRoom;
use sts2_core::run::{RunDecision, RunState};

pub use crate::obs_full::{FULL_AT, FULL_BYTES};
use crate::run_action::OPTION_SLOTS;
use crate::vocab;

/// Options per page.
pub const OPTIONS: usize = OPTION_SLOTS;

/// The number columns, in order.
pub const COLUMN_NAMES: [&str; 32] = [
    "known",
    "hp",
    "heal_full",
    "max_hp",
    "gold",
    "gold_all",
    "cards",
    "curses",
    "card_rewards",
    "choices",
    "remove",
    "upgrade",
    "downgrade",
    "transform",
    "enchant",
    "enchant_amount",
    "all",
    "duplicate",
    "relics",
    "relic_rarity",
    "relics_lost",
    "potions",
    "potions_lost",
    "potion_slots",
    "fight",
    "foe_hp",
    "random",
    "choose",
    "more",
    "delayed",
    "minigame",
    "leave",
];
/// Number columns per option.
pub const COLUMNS: usize = COLUMN_NAMES.len();

/// The id slots, in order: what the option gives, then what it takes.
pub const ID_SLOT_NAMES: [&str; 7] = [
    "card",
    "curse",
    "relic",
    "potion",
    "lost_card",
    "lost_relic",
    "lost_potion",
];
/// Id slots per option.
pub const ID_SLOTS: usize = ID_SLOT_NAMES.len();

/// Bytes per option: the numbers, the ids, the enchantment and 0.
pub const OPTION_BYTES: usize = COLUMNS * 2 + ID_SLOTS * 2 + 2;
/// Bytes of the section.
pub const EVENTS_BYTES: usize = OPTIONS * OPTION_BYTES;
/// The section's offset in the run observation.
pub const EVENTS_AT: usize = FULL_AT + FULL_BYTES;

/// An event option's spec, by its text key (module doc).
///
/// `@RELIC` is an option that obtains a
/// relic whose own text it shows ([`RELIC_SPECS`]). A locked option (no callback) gets `known`
/// and nothing else, whatever its spec.
pub const EVENT_SPECS: &[(&str, &str)] = &[
    ("ABYSSAL_BATHS.pages.ALL.options.EXIT_BATHS", "leave"),
    (
        "ABYSSAL_BATHS.pages.ALL.options.LINGER",
        "max_hp=$MaxHp hp=-$Damage more",
    ),
    ("ABYSSAL_BATHS.pages.INITIAL.options.ABSTAIN", "hp=$Heal"),
    (
        "ABYSSAL_BATHS.pages.INITIAL.options.IMMERSE",
        "max_hp=$MaxHp hp=-$Damage more",
    ),
    (
        "AMALGAMATOR.pages.INITIAL.options.COMBINE_DEFENDS",
        "remove=2 cards=1 card=$Card2",
    ),
    (
        "AMALGAMATOR.pages.INITIAL.options.COMBINE_STRIKES",
        "remove=2 cards=1 card=$Card1",
    ),
    (
        "AROMA_OF_CHAOS.pages.INITIAL.options.LET_GO",
        "transform=1 choose",
    ),
    (
        "AROMA_OF_CHAOS.pages.INITIAL.options.MAINTAIN_CONTROL",
        "upgrade=1 choose",
    ),
    (
        "BATTLEWORN_DUMMY.pages.INITIAL.options.SETTING_1",
        "fight foe_hp=$Setting1Hp potions=1 random",
    ),
    (
        "BATTLEWORN_DUMMY.pages.INITIAL.options.SETTING_2",
        "fight foe_hp=$Setting2Hp upgrade=2 random",
    ),
    (
        "BATTLEWORN_DUMMY.pages.INITIAL.options.SETTING_3",
        "fight foe_hp=$Setting3Hp relics=1 random",
    ),
    (
        "BRAIN_LEECH.pages.INITIAL.options.RIP",
        "hp=-$RipHpLoss card_rewards=$RewardCount random",
    ),
    (
        "BRAIN_LEECH.pages.INITIAL.options.SHARE_KNOWLEDGE",
        "cards=$CardChoiceCount choices=$FromCardChoiceCount random",
    ),
    (
        "BUGSLAYER.pages.INITIAL.options.EXTERMINATION",
        "cards=1 card=$Card1",
    ),
    (
        "BUGSLAYER.pages.INITIAL.options.SQUASH",
        "cards=1 card=$Card2",
    ),
    ("BYRDONIS_NEST.pages.INITIAL.options.EAT", "max_hp=$MaxHp"),
    (
        "BYRDONIS_NEST.pages.INITIAL.options.TAKE",
        "cards=1 card=$Card",
    ),
    (
        "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.DEFECT",
        "card_rewards=$Cards random",
    ),
    (
        "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.IRONCLAD",
        "card_rewards=$Cards random",
    ),
    (
        "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.NECROBINDER",
        "card_rewards=$Cards random",
    ),
    (
        "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.REGENT",
        "card_rewards=$Cards random",
    ),
    (
        "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.SILENT",
        "card_rewards=$Cards random",
    ),
    (
        "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.EQUALITY",
        "relics=1 relic=PRISMATIC_SHARD",
    ),
    (
        "COLOSSAL_FLOWER.pages.INITIAL.options.EXTRACT_CURRENT_PRIZE_1",
        "gold=$Prize1",
    ),
    (
        "COLOSSAL_FLOWER.pages.INITIAL.options.REACH_DEEPER_1",
        "hp=-5 more",
    ),
    (
        "COLOSSAL_FLOWER.pages.REACH_DEEPER_1.options.EXTRACT_CURRENT_PRIZE_2",
        "gold=$Prize2",
    ),
    (
        "COLOSSAL_FLOWER.pages.REACH_DEEPER_1.options.REACH_DEEPER_2",
        "hp=-6 more",
    ),
    (
        "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.EXTRACT_INSTEAD",
        "gold=$Prize3",
    ),
    (
        "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.POLLINOUS_CORE",
        "hp=-7 relics=1 relic=POLLINOUS_CORE",
    ),
    (
        "CRYSTAL_SPHERE.pages.INITIAL.options.PAYMENT_PLAN",
        "curses=1 curse=$CurseTitle minigame=$PaymentPlanCount",
    ),
    (
        "CRYSTAL_SPHERE.pages.INITIAL.options.UNCOVER_FUTURE",
        "gold=-$UncoverFutureCost minigame=$UncoverFutureProphesizeCount",
    ),
    (
        "DENSE_VEGETATION.pages.INITIAL.options.REST",
        "hp=$Heal fight more",
    ),
    (
        "DENSE_VEGETATION.pages.INITIAL.options.TRUDGE_ON",
        "gold=$Gold hp=-$HpLoss",
    ),
    ("DENSE_VEGETATION.pages.REST.options.FIGHT", "fight"),
    (
        "DOLL_ROOM.pages.INITIAL.options.EXAMINE",
        "hp=-$ExamineHpLoss choices=3 more",
    ),
    ("DOLL_ROOM.pages.INITIAL.options.RANDOM", "relics=1 random"),
    (
        "DOLL_ROOM.pages.INITIAL.options.TAKE_SOME_TIME",
        "hp=-$TakeTimeHpLoss choices=2 more",
    ),
    (
        "DOORS_OF_LIGHT_AND_DARK.pages.INITIAL.options.DARK",
        "remove=1 choose",
    ),
    (
        "DOORS_OF_LIGHT_AND_DARK.pages.INITIAL.options.LIGHT",
        "upgrade=$Cards random",
    ),
    (
        "DROWNING_BEACON.pages.INITIAL.options.BOTTLE",
        "potions=1 potion=$Potion",
    ),
    (
        "DROWNING_BEACON.pages.INITIAL.options.CLIMB",
        "relics=1 relic=$Relic max_hp=-$HpLoss",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.CAVIAR",
        "gold=-$Gold max_hp=$CaviarMaxHp more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.CLAM_ROLL",
        "gold=-$Gold hp=$ClamRollHeal more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.FRIED_EEL",
        "gold=-$Gold cards=1 random more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.GOLDEN_FYSH",
        "gold=$GoldenFyshGold more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.JELLY_LIVER",
        "gold=-$Gold transform=1 choose more",
    ),
    ("ENDLESS_CONVEYOR.pages.ALL.options.LOCKED", ""),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.SEAPUNK_SALAD",
        "gold=-$Gold cards=1 card=FEEDING_FRENZY more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.SPICY_SNAPPY",
        "gold=-$Gold upgrade=1 random more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.ALL.options.SUSPICIOUS_CONDIMENT",
        "gold=-$Gold potions=1 random more",
    ),
    (
        "ENDLESS_CONVEYOR.pages.GRAB_SOMETHING_OFF_THE_BELT.options.LEAVE",
        "leave",
    ),
    (
        "ENDLESS_CONVEYOR.pages.INITIAL.options.OBSERVE_CHEF",
        "upgrade=1 random",
    ),
    (
        "FIELD_OF_MAN_SIZED_HOLES.pages.INITIAL.options.ENTER_YOUR_HOLE",
        "enchant=1 ench=$Enchantment choose",
    ),
    (
        "FIELD_OF_MAN_SIZED_HOLES.pages.INITIAL.options.RESIST",
        "remove=$Cards choose curses=1 curse=$ResistCurse",
    ),
    (
        "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.ACCEPT",
        "relics=1 relic=$Relic",
    ),
    (
        "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.CONFRONT",
        "curses=1 curse=$Curse enchant=1 ench=$Enchantment choose",
    ),
    (
        "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.CONFRONT_LOCKED",
        "",
    ),
    (
        "HUNGRY_FOR_MUSHROOMS.pages.INITIAL.options.BIG_MUSHROOM",
        "@BIG_MUSHROOM",
    ),
    (
        "HUNGRY_FOR_MUSHROOMS.pages.INITIAL.options.FRAGRANT_MUSHROOM",
        "@FRAGRANT_MUSHROOM",
    ),
    (
        "INFESTED_AUTOMATON.pages.INITIAL.options.STUDY",
        "cards=1 random",
    ),
    (
        "INFESTED_AUTOMATON.pages.INITIAL.options.TOUCH_CORE",
        "cards=1 random",
    ),
    (
        "JUNGLE_MAZE_ADVENTURE.pages.INITIAL.options.JOIN_FORCES",
        "gold=$JoinForcesGold",
    ),
    (
        "JUNGLE_MAZE_ADVENTURE.pages.INITIAL.options.SOLO_QUEST",
        "gold=$SoloGold hp=-$SoloHp",
    ),
    (
        "LOST_WISP.pages.INITIAL.options.CLAIM",
        "curses=1 curse=$Curse relics=1 relic=$Relic",
    ),
    ("LOST_WISP.pages.INITIAL.options.CLAIM_LOCKED", ""),
    ("LOST_WISP.pages.INITIAL.options.SEARCH", "gold=$Gold"),
    (
        "LUMINOUS_CHOIR.pages.INITIAL.options.OFFER_TRIBUTE",
        "gold=-$Gold relics=1 random",
    ),
    (
        "LUMINOUS_CHOIR.pages.INITIAL.options.OFFER_TRIBUTE_LOCKED",
        "",
    ),
    (
        "LUMINOUS_CHOIR.pages.INITIAL.options.REACH_INTO_THE_FLESH",
        "remove=2 choose curses=1 curse=SPORE_MIND",
    ),
    (
        "MORPHIC_GROVE.pages.INITIAL.options.GROUP",
        "gold_all transform=2 choose",
    ),
    ("MORPHIC_GROVE.pages.INITIAL.options.LONER", "max_hp=$MaxHp"),
    (
        "POTION_COURIER.pages.INITIAL.options.GRAB_POTIONS",
        "potions=$FoulPotions potion=FOUL_POTION",
    ),
    (
        "POTION_COURIER.pages.INITIAL.options.RANSACK",
        "potions=1 random",
    ),
    (
        "PUNCH_OFF.pages.INITIAL.options.I_CAN_TAKE_THEM",
        "fight more",
    ),
    (
        "PUNCH_OFF.pages.INITIAL.options.NAB",
        "curses=1 curse=INJURY relics=1 random",
    ),
    ("PUNCH_OFF.pages.I_CAN_TAKE_THEM.options.FIGHT", "fight"),
    (
        "RANWID_THE_ELDER.pages.INITIAL.options.GOLD",
        "gold=-$Gold relics=1 random",
    ),
    (
        "RANWID_THE_ELDER.pages.INITIAL.options.POTION",
        "potions_lost=1 relics=1 random",
    ),
    ("RANWID_THE_ELDER.pages.INITIAL.options.POTION_LOCKED", ""),
    (
        "RANWID_THE_ELDER.pages.INITIAL.options.RELIC",
        "relics_lost=1 relics=2 random",
    ),
    ("RANWID_THE_ELDER.pages.INITIAL.options.RELIC_LOCKED", ""),
    (
        "REFLECTIONS.pages.INITIAL.options.SHATTER",
        "duplicate curses=1 curse=BAD_LUCK",
    ),
    (
        "REFLECTIONS.pages.INITIAL.options.TOUCH_A_MIRROR",
        "downgrade=2 upgrade=4 random",
    ),
    (
        "RELIC_TRADER.pages.INITIAL.options.BOTTOM",
        "relics_lost=1 lost_relic=$BottomRelicOwned relics=1 relic=$BottomRelicNew",
    ),
    (
        "RELIC_TRADER.pages.INITIAL.options.MIDDLE",
        "relics_lost=1 lost_relic=$MiddleRelicOwned relics=1 relic=$MiddleRelicNew",
    ),
    (
        "RELIC_TRADER.pages.INITIAL.options.TOP",
        "relics_lost=1 lost_relic=$TopRelicOwned relics=1 relic=$TopRelicNew",
    ),
    (
        "ROOM_FULL_OF_CHEESE.pages.INITIAL.options.GORGE",
        "cards=2 choices=8 random",
    ),
    (
        "ROOM_FULL_OF_CHEESE.pages.INITIAL.options.SEARCH",
        "hp=-$Damage relics=1 relic=CHOSEN_CHEESE",
    ),
    (
        "ROUND_TEA_PARTY.pages.INITIAL.options.ENJOY_TEA",
        "relics=1 relic=$Relic heal_full",
    ),
    (
        "ROUND_TEA_PARTY.pages.INITIAL.options.PICK_FIGHT",
        "hp=-$Damage relics=1 random more",
    ),
    (
        "ROUND_TEA_PARTY.pages.PICK_FIGHT.options.CONTINUE_FIGHT",
        "hp=-$Damage relics=1 random",
    ),
    (
        "SAPPHIRE_SEED.pages.INITIAL.options.EAT",
        "hp=$Heal upgrade=1 choose",
    ),
    (
        "SAPPHIRE_SEED.pages.INITIAL.options.PLANT",
        "enchant=1 ench=$Enchantment choose",
    ),
    ("SELF_HELP_BOOK.pages.INITIAL.options.NO_OPTIONS", "leave"),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_ENTIRE_BOOK",
        "enchant=1 ench=$Enchantment3 enchant_amount=$Enchantment3Amount choose",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_ENTIRE_BOOK_LOCKED",
        "",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_PASSAGE",
        "enchant=1 ench=$Enchantment2 enchant_amount=$Enchantment2Amount choose",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_PASSAGE_LOCKED",
        "",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_THE_BACK",
        "enchant=1 ench=$Enchantment1 enchant_amount=$Enchantment1Amount choose",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_THE_BACK_LOCKED",
        "",
    ),
    (
        "SLIPPERY_BRIDGE.pages.INITIAL.options.OVERCOME",
        "remove=1 lost_card=$RandomCard",
    ),
    (
        "SLIPPERY_BRIDGE.pages.*.options.HOLD_ON_*",
        "hp=-$HpLoss random more",
    ),
    (
        "SPIRALING_WHIRLPOOL.pages.INITIAL.options.DRINK",
        "hp=$Heal",
    ),
    (
        "SPIRALING_WHIRLPOOL.pages.INITIAL.options.OBSERVE",
        "enchant=1 ench=SPIRAL choose",
    ),
    (
        "SPIRALING_WHIRLPOOL.pages.INITIAL.options.REACH_IN",
        "relics=1 random curses=1 curse=WRITHE",
    ),
    (
        "SPIRIT_GRAFTER.pages.INITIAL.options.LET_IT_IN",
        "hp=$LetItInHealAmount cards=1 card=METAMORPHOSIS",
    ),
    (
        "SPIRIT_GRAFTER.pages.INITIAL.options.REJECTION",
        "hp=-$RejectionHpLoss upgrade=1 choose",
    ),
    (
        "STONE_OF_ALL_TIME.pages.INITIAL.options.LIFT",
        "potions_lost=1 lost_potion=$DrinkRandomPotion max_hp=$DrinkMaxHpGain",
    ),
    ("STONE_OF_ALL_TIME.pages.INITIAL.options.LIFT_LOCKED", ""),
    (
        "STONE_OF_ALL_TIME.pages.INITIAL.options.PUSH",
        "hp=-$PushHpLoss enchant=1 ench=VIGOROUS enchant_amount=$PushVigorousAmount choose",
    ),
    ("STONE_OF_ALL_TIME.pages.INITIAL.options.PUSH_LOCKED", ""),
    (
        "SUNKEN_STATUE.pages.INITIAL.options.DIVE_INTO_WATER",
        "gold=$Gold hp=-$HpLoss",
    ),
    (
        "SUNKEN_STATUE.pages.INITIAL.options.GRAB_SWORD",
        "relics=1 relic=$Relic",
    ),
    (
        "SUNKEN_TREASURY.pages.INITIAL.options.FIRST_CHEST",
        "gold=$SmallChestGold",
    ),
    (
        "SUNKEN_TREASURY.pages.INITIAL.options.SECOND_CHEST",
        "gold=$LargeChestGold curses=1 curse=GREED",
    ),
    (
        "SYMBIOTE.pages.INITIAL.options.APPROACH",
        "enchant=1 ench=$Enchantment choose",
    ),
    ("SYMBIOTE.pages.INITIAL.options.APPROACH_LOCKED", ""),
    (
        "SYMBIOTE.pages.INITIAL.options.KILL_WITH_FIRE",
        "transform=1 choose",
    ),
    ("TABLET_OF_TRUTH.pages.DECIPHER.options.GIVE_UP", "leave"),
    (
        "TABLET_OF_TRUTH.pages.DECIPHER_1.options.DECIPHER",
        "max_hp=-$DecipherMaxHpLoss upgrade=1 random more",
    ),
    (
        "TABLET_OF_TRUTH.pages.DECIPHER_2.options.DECIPHER",
        "max_hp=-$DecipherMaxHpLoss upgrade=1 random more",
    ),
    (
        "TABLET_OF_TRUTH.pages.DECIPHER_3.options.DECIPHER",
        "max_hp=-$DecipherMaxHpLoss upgrade=1 random more",
    ),
    (
        "TABLET_OF_TRUTH.pages.DECIPHER_4.options.DECIPHER",
        "max_hp=-$DecipherMaxHpLoss upgrade=1 all",
    ),
    (
        "TABLET_OF_TRUTH.pages.INITIAL.options.DECIPHER_1",
        "max_hp=-$DecipherMaxHpLoss upgrade=1 random more",
    ),
    (
        "TABLET_OF_TRUTH.pages.INITIAL.options.SMASH",
        "hp=$SmashHPGain",
    ),
    (
        "TEA_MASTER.pages.INITIAL.options.BONE_TEA",
        "gold=-$BoneTeaCost relics=1 relic=BONE_TEA",
    ),
    ("TEA_MASTER.pages.INITIAL.options.BONE_TEA_LOCKED", ""),
    (
        "TEA_MASTER.pages.INITIAL.options.EMBER_TEA",
        "gold=-$EmberTeaCost relics=1 relic=EMBER_TEA",
    ),
    ("TEA_MASTER.pages.INITIAL.options.EMBER_TEA_LOCKED", ""),
    (
        "TEA_MASTER.pages.INITIAL.options.TEA_OF_DISCOURTESY",
        "relics=1 relic=TEA_OF_DISCOURTESY",
    ),
    (
        "THE_LANTERN_KEY.pages.INITIAL.options.KEEP_THE_KEY",
        "fight more",
    ),
    (
        "THE_LANTERN_KEY.pages.INITIAL.options.RETURN_THE_KEY",
        "gold=$Gold",
    ),
    (
        "THE_LANTERN_KEY.pages.KEEP_THE_KEY.options.FIGHT",
        "fight cards=1 card=LANTERN_KEY",
    ),
    (
        "THE_LEGENDS_WERE_TRUE.pages.INITIAL.options.NAB_THE_MAP",
        "cards=1 card=SPOILS_MAP",
    ),
    (
        "THE_LEGENDS_WERE_TRUE.pages.INITIAL.options.SLOWLY_FIND_AN_EXIT",
        "hp=-$Damage potions=1 random",
    ),
    (
        "THIS_OR_THAT.pages.INITIAL.options.ORNATE",
        "curses=1 curse=$Curse relics=1 random",
    ),
    (
        "THIS_OR_THAT.pages.INITIAL.options.PLAIN",
        "hp=-$HpLoss gold=$Gold",
    ),
    (
        "TINKER_TIME.pages.INITIAL.options.CHOOSE_CARD_TYPE",
        "cards=1 more",
    ),
    ("TINKER_TIME.pages.CHOOSE_CARD_TYPE.options.ATTACK", "more"),
    ("TINKER_TIME.pages.CHOOSE_CARD_TYPE.options.POWER", "more"),
    ("TINKER_TIME.pages.CHOOSE_CARD_TYPE.options.SKILL", "more"),
    (
        "TRASH_HEAP.pages.INITIAL.options.DIVE_IN",
        "hp=-$HpLoss relics=1 random",
    ),
    (
        "TRASH_HEAP.pages.INITIAL.options.GRAB",
        "gold=$Gold cards=1 random",
    ),
    ("TRIAL.pages.INITIAL.options.ACCEPT", "more"),
    ("TRIAL.pages.INITIAL.options.REJECT", "more"),
    (
        "TRIAL.pages.MERCHANT.options.GUILTY",
        "curses=1 curse=REGRET relics=2 random",
    ),
    (
        "TRIAL.pages.MERCHANT.options.INNOCENT",
        "curses=1 curse=SHAME upgrade=2 choose",
    ),
    ("TRIAL.pages.NOBLE.options.GUILTY", "hp=10"),
    (
        "TRIAL.pages.NOBLE.options.INNOCENT",
        "curses=1 curse=REGRET gold=300",
    ),
    (
        "TRIAL.pages.NONDESCRIPT.options.GUILTY",
        "curses=1 curse=DOUBT card_rewards=2 random",
    ),
    (
        "TRIAL.pages.NONDESCRIPT.options.INNOCENT",
        "curses=1 curse=DOUBT transform=2 choose",
    ),
    ("TRIAL.pages.REJECT.options.ACCEPT", "more"),
    ("TRIAL.pages.REJECT.options.DOUBLE_DOWN", "more"),
    (
        "UNREST_SITE.pages.INITIAL.options.KILL",
        "max_hp=-$MaxHpLoss relics=1 random",
    ),
    (
        "UNREST_SITE.pages.INITIAL.options.REST",
        "heal_full curses=1 curse=POOR_SLEEP",
    ),
    (
        "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CAGE",
        "lost_card=LANTERN_KEY relics=1 relic=HISTORY_COURSE",
    ),
    (
        "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CHEST",
        "lost_card=LANTERN_KEY potions=2 relics=2 random",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.BLOODY_INK",
        "max_hp=$MaxHp",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.PRICKLY_SPONGE",
        "gold=-$PricklySpongeGold enchant=$Cards ench=STEADY choose",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.PRICKLY_SPONGE_LOCKED",
        "",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.TENTACLE_QUILL",
        "gold=-$Gold enchant=1 ench=STEADY choose",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.TENTACLE_QUILL_LOCKED",
        "",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.BARGAIN_BIN",
        "gold=-$BargainBinCost relics=1 relic_rarity=1 random",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.BARGAIN_BIN_LOCKED",
        "",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.FEATURED_ITEM",
        "gold=-$FeaturedItemCost relics=1 relic=$RandomRelic",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.FEATURED_ITEM_LOCKED",
        "",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.LEAVE",
        "downgrade=1 random",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.MYSTERY_BOX",
        "gold=-$MysteryBoxCost relics=$MysteryBoxRelicCount random delayed=$MysteryBoxCombatCount",
    ),
    (
        "WELCOME_TO_WONGOS.pages.INITIAL.options.MYSTERY_BOX_LOCKED",
        "",
    ),
    (
        "WELLSPRING.pages.INITIAL.options.BATHE",
        "remove=1 choose curses=$BatheCurses curse=GUILTY",
    ),
    (
        "WELLSPRING.pages.INITIAL.options.BOTTLE",
        "potions=1 random",
    ),
    (
        "WHISPERING_HOLLOW.pages.INITIAL.options.GOLD",
        "gold=-$Gold potions=2 random",
    ),
    (
        "WHISPERING_HOLLOW.pages.INITIAL.options.HUG",
        "hp=-$HpLoss transform=1 choose",
    ),
    (
        "WOOD_CARVINGS.pages.INITIAL.options.BIRD",
        "transform=1 choose card=$BirdCard",
    ),
    (
        "WOOD_CARVINGS.pages.INITIAL.options.SNAKE",
        "enchant=1 ench=$SnakeEnchantment choose",
    ),
    ("WOOD_CARVINGS.pages.INITIAL.options.SNAKE_LOCKED", ""),
    (
        "WOOD_CARVINGS.pages.INITIAL.options.TORUS",
        "transform=1 choose card=$ToricCard",
    ),
    (
        "ZEN_WEAVER.pages.INITIAL.options.ARACHNID_ACUPUNCTURE",
        "gold=-$ArachnidAcupunctureCost remove=2 choose",
    ),
    (
        "ZEN_WEAVER.pages.INITIAL.options.BREATHING_TECHNIQUES",
        "gold=-$BreathingTechniquesCost cards=2 card=ENLIGHTENMENT",
    ),
    (
        "ZEN_WEAVER.pages.INITIAL.options.EMOTIONAL_AWARENESS",
        "gold=-$EmotionalAwarenessCost remove=1 choose",
    ),
    ("ZEN_WEAVER.pages.INITIAL.options.LOCKED", ""),
    ("FAKE_MERCHANT.pages.INITIAL.options.LEAVE", "leave"),
    ("PROCEED", "leave"),
    (
        "FAKE_MERCHANT.pages.INITIAL.options.FOUL_POTION",
        "fight potions_lost=1 lost_potion=FOUL_POTION",
    ),
];

/// The options whose text needs more than the vector has (module doc).
///
/// They are written with `known` 0 and what they could fill. `TinkerTime`'s riders describe the card being made (its damage,
/// draw, energy, powers); `TheFutureOfPotions` names the card's rarity and type.
pub const GAPS: &[(&str, &str)] = &[
    ("TINKER_TIME.pages.CHOOSE_RIDER.options.*", "more"),
    (
        "THE_FUTURE_OF_POTIONS.pages.INITIAL.options.POTION",
        "potions_lost=1 cards=1 random",
    ),
    ("CRYSTAL_SPHERE.minigame.*", "minigame=1"),
];

/// A relic's pickup effects (`Upon pickup, ...`), by id.
///
/// For the relics an ancient or a relic event offers; `$Name` is the relic's canonical var. `""` for a relic with none (its effect is
/// the relic itself: its id and content stats). A relic option also gets `relics=1` and its id.
pub const RELIC_SPECS: &[(&str, &str)] = &[
    (
        "ALCHEMICAL_COFFER",
        "potion_slots=$PotionSlots potions=$PotionSlots random",
    ),
    ("ARCANE_SCROLL", "cards=1 random"),
    ("ARCHAIC_TOOTH", "transform=1"),
    ("ASTROLABE", "transform=$Cards upgrade=$Cards choose"),
    (
        "BEAUTIFUL_BRACELET",
        "enchant=$Cards ench=SWIFT enchant_amount=2 random",
    ),
    ("BIG_MUSHROOM", "max_hp=$MaxHp"),
    ("BIIIG_HUG", "remove=$Cards choose"),
    ("BING_BONG", ""),
    ("BLACK_STAR", ""),
    ("BLESSED_ANTLER", ""),
    ("BLOOD_SOAKED_ROSE", "curses=1 curse=ENTHRALLED"),
    ("BOOMING_CONCH", ""),
    ("BRILLIANT_SCARF", ""),
    ("CALLING_BELL", "curses=1 relics=$Relics random"),
    ("CHOICES_PARADOX", ""),
    ("CLAWS", "transform=$Cards card=MAUL choose"),
    ("CROSSBOW", ""),
    ("CURSED_PEARL", "curses=1 curse=GREED gold=$Gold"),
    ("DAUGHTER_OF_THE_WIND", ""),
    ("DELICATE_FROND", ""),
    ("DIAMOND_DIADEM", ""),
    (
        "DISTINGUISHED_CAPE",
        "curses=$Curses cards=$Cards card=APPARITION random",
    ),
    ("DOWSING_ROD", "cards=1 card=DOWSING"),
    ("DRIFTWOOD", ""),
    ("DUSTY_TOME", "cards=1 random"),
    ("ECTOPLASM", ""),
    ("ELECTRIC_SHRYMP", "enchant=1 ench=IMBUED choose"),
    ("EMPTY_CAGE", "remove=$Cards choose"),
    ("FIDDLE", ""),
    ("FISHING_ROD", ""),
    ("FRAGRANT_MUSHROOM", "hp=-$HpLoss upgrade=$Cards random"),
    ("FUR_COAT", ""),
    ("GLASS_EYE", "card_rewards=5 random"),
    ("GLITTER", ""),
    ("GOLDEN_COMPASS", ""),
    ("GOLDEN_PEARL", "gold=$Gold"),
    (
        "HEFTY_TABLET",
        "cards=1 choices=$Cards random curses=1 curse=INJURY",
    ),
    ("IRON_CLUB", ""),
    ("JEWELED_MASK", ""),
    ("JEWELRY_BOX", "cards=1 card=APOTHEOSIS"),
    ("KALEIDOSCOPE", "card_rewards=$Cards random"),
    ("LARGE_CAPSULE", "relics=$Relics random cards=2"),
    ("LAVA_ROCK", ""),
    ("LEAD_PAPERWEIGHT", "cards=1 choices=2 random"),
    ("LEAFY_POULTICE", "transform=2 max_hp=-$MaxHp"),
    ("LOOMING_FRUIT", "max_hp=$MaxHp"),
    ("LORDS_PARASOL", ""),
    ("LOST_COFFER", "card_rewards=1 potions=1 random"),
    ("MEAT_CLEAVER", ""),
    ("MR_STRUGGLES", ""),
    ("MUSIC_BOX", ""),
    ("NEOWS_BONES", "relics=$Relics curses=$Curses random"),
    (
        "NEOWS_SACRIFICE",
        "potions=1 potion=AMBERGRIS curses=1 curse=GUILTY",
    ),
    ("NEOWS_TALISMAN", "upgrade=2"),
    ("NEOWS_TORMENT", "cards=1 card=NEOWS_FURY"),
    ("NEW_LEAF", "transform=$Cards choose"),
    ("NUTRITIOUS_OYSTER", "max_hp=$MaxHp"),
    ("NUTRITIOUS_SOUP", "enchant=1 all ench=TEZCATARAS_EMBER"),
    ("PAELS_BLOOD", ""),
    ("PAELS_CLAW", "enchant=1 all ench=GOOPY"),
    ("PAELS_EYE", ""),
    ("PAELS_FLESH", ""),
    ("PAELS_GROWTH", "enchant=1 ench=CLONE choose"),
    ("PAELS_HORN", "cards=2 card=RELAX"),
    ("PAELS_LEGION", ""),
    ("PAELS_TEARS", ""),
    ("PAELS_TOOTH", "remove=$Cards choose"),
    ("PAELS_WING", ""),
    ("PANDORAS_BOX", "transform=1 all random"),
    (
        "PHIAL_HOLSTER",
        "potion_slots=$PotionSlots potions=$Potions random",
    ),
    ("PHILOSOPHERS_STONE", ""),
    ("POMANDER", "upgrade=1 choose"),
    ("PRECARIOUS_SHEARS", "remove=$Cards choose hp=-$Damage"),
    ("PRECISE_SCISSORS", "remove=$Cards choose"),
    ("PRESERVED_FOG", "remove=$Cards choose curses=1 curse=FOLLY"),
    ("PRISMATIC_GEM", ""),
    ("PUMPKIN_CANDLE", ""),
    ("RADIANT_PEARL", ""),
    ("RUNIC_PYRAMID", ""),
    ("SAND_CASTLE", "upgrade=$Cards random"),
    ("SCROLL_BOXES", "choices=2 random"),
    ("SEA_GLASS", "choices=$Cards random"),
    ("SEAL_OF_GOLD", ""),
    ("SERE_TALON", "max_hp=-9 cards=$Wishes card=WISH"),
    ("SIGNET_RING", "gold=$Gold"),
    ("SILKEN_TRESS", "gold_all"),
    ("SILVER_CRUCIBLE", ""),
    ("SMALL_CAPSULE", "relics=1 random"),
    ("SNECKO_EYE", ""),
    ("SOZU", ""),
    ("SPIKED_GAUNTLETS", ""),
    ("STONE_HUMIDIFIER", ""),
    ("STORYBOOK", "cards=1 card=BRIGHTEST_FLAME"),
    ("TANXS_WHISTLE", "cards=1 card=WHISTLE"),
    ("THROWING_AXE", ""),
    ("TOASTY_MITTENS", ""),
    ("TOUCH_OF_OROBAS", "relics_lost=1"),
    ("TOY_BOX", "relics=$Relics random"),
    ("TRI_BOOMERANG", "enchant=$Cards ench=INSTINCT choose"),
    ("VELVET_CHOKER", ""),
    ("VERY_HOT_COCOA", ""),
    ("WAR_HAMMER", ""),
    ("WHISPERING_EARRING", ""),
    ("WINGED_BOOTS", ""),
    ("YUMMY_COOKIE", "upgrade=$Cards choose"),
];

/// Where a spec's number or id comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    /// A literal.
    Lit(i32),
    /// A literal id.
    Id(&'static str),
    /// A var, negated if `neg`.
    Var { name: &'static str, neg: bool },
}

/// A parsed spec: (column or id slot index, source) pairs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spec {
    columns: Vec<(usize, Source)>,
    ids: Vec<(usize, Source)>,
    ench: Option<Source>,
    /// `@RELIC`: a relic option.
    relic: Option<&'static str>,
}

/// The enchantment pseudo-slot's name in a spec.
const ENCH: &str = "ench";

/// Parses `spec` (module doc).
///
/// # Errors
/// A token that is no column, id slot or flag, or a value that does not parse.
pub fn parse(spec: &'static str) -> Result<Spec, String> {
    let mut out = Spec::default();
    if let Some(relic) = spec.strip_prefix('@') {
        out.relic = Some(relic);
        return Ok(out);
    }
    for token in spec.split_whitespace() {
        let (name, value) = token.split_once('=').unwrap_or((token, "1"));
        let source = match (value.strip_prefix("-$"), value.strip_prefix('$')) {
            (Some(name), _) => Source::Var { name, neg: true },
            (None, Some(name)) => Source::Var { name, neg: false },
            (None, None) => value.parse::<i32>().map_or(Source::Id(value), Source::Lit),
        };
        if name == ENCH {
            out.ench = Some(source);
        } else if let Some(slot) = ID_SLOT_NAMES.iter().position(|&s| s == name) {
            out.ids.push((slot, source));
        } else if let Some(col) = COLUMN_NAMES.iter().position(|&c| c == name) {
            if matches!(source, Source::Id(_)) || col == 0 {
                return Err(format!("{token}: not a number"));
            }
            out.columns.push((col, source));
        } else {
            return Err(format!("{token}: no such column or slot"));
        }
    }
    Ok(out)
}

/// Whether text key `key` matches `pattern` (`*` matches one dot-free part or its rest).
fn matches(pattern: &str, key: &str) -> bool {
    let (mut p, mut k) = (pattern.split('.'), key.split('.'));
    loop {
        match (p.next(), k.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) => {
                let ok = a
                    .strip_suffix('*')
                    .map_or_else(|| a == b, |prefix| b.starts_with(prefix));
                if !ok {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// The parsed specs: exact keys, patterns, and the relics'.
struct Specs {
    exact: HashMap<&'static str, (Spec, bool)>,
    patterns: Vec<(&'static str, Spec, bool)>,
    relics: HashMap<&'static str, Spec>,
}

fn specs() -> &'static Specs {
    static SPECS: OnceLock<Specs> = OnceLock::new();
    SPECS.get_or_init(|| {
        let parse = |s| parse(s).unwrap_or_else(|e| panic!("bad spec {s:?}: {e}"));
        let mut exact = HashMap::new();
        let mut patterns = Vec::new();
        for (table, known) in [(EVENT_SPECS, true), (GAPS, false)] {
            for &(key, spec) in table {
                if key.contains('*') {
                    patterns.push((key, parse(spec), known));
                } else {
                    exact.insert(key, (parse(spec), known));
                }
            }
        }
        let relics = RELIC_SPECS.iter().map(|&(id, s)| (id, parse(s))).collect();
        Specs {
            exact,
            patterns,
            relics,
        }
    })
}

/// The spec of event option text key `key`, and whether it is complete; `None` if there is
/// none.
fn event_spec(key: &str) -> Option<(&'static Spec, bool)> {
    let s = specs();
    s.exact
        .get(key)
        .map(|(spec, known)| (spec, *known))
        .or_else(|| {
            s.patterns
                .iter()
                .find(|(p, _, _)| matches(p, key))
                .map(|(_, spec, known)| (spec, *known))
        })
}

/// One option's vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionVector {
    /// The numbers ([`COLUMN_NAMES`]).
    pub columns: [i32; COLUMNS],
    /// The vocab ids ([`ID_SLOT_NAMES`]).
    pub ids: [u16; ID_SLOTS],
    /// The enchantment (`EnchantmentKind` index + 1, 0 none).
    pub ench: u8,
}

impl Default for OptionVector {
    fn default() -> Self {
        Self {
            columns: [0; COLUMNS],
            ids: [0; ID_SLOTS],
            ench: 0,
        }
    }
}

impl OptionVector {
    /// The value of column `name`.
    ///
    /// # Panics
    /// If there is no such column.
    #[must_use]
    pub fn get(&self, name: &str) -> i32 {
        let col = COLUMN_NAMES
            .iter()
            .position(|&c| c == name)
            .unwrap_or_else(|| panic!("no column {name}"));
        self.columns[col]
    }

    /// The vocab id in slot `name`.
    ///
    /// # Panics
    /// If there is no such slot.
    #[must_use]
    pub fn id(&self, name: &str) -> u16 {
        let slot = ID_SLOT_NAMES
            .iter()
            .position(|&c| c == name)
            .unwrap_or_else(|| panic!("no slot {name}"));
        self.ids[slot]
    }

    /// Whether the vector holds everything the option's text states.
    #[must_use]
    pub const fn known(&self) -> bool {
        self.columns[0] != 0
    }

    fn write(&self, out: &mut [u8]) {
        let mut at = 0;
        for &v in &self.columns {
            let v = i16::try_from(v).unwrap_or(if v < 0 { i16::MIN } else { i16::MAX });
            out[at..at + 2].copy_from_slice(&v.to_le_bytes());
            at += 2;
        }
        for &id in &self.ids {
            out[at..at + 2].copy_from_slice(&id.to_le_bytes());
            at += 2;
        }
        out[at] = self.ench;
    }
}

/// A var's value: a number, or the id a `StringVar` names.
#[derive(Clone, Copy, Debug)]
enum Value {
    Number(i32),
    Id(&'static str),
}

/// Fills `v` from `spec`, looking vars up with `var`; returns whether every var had a value.
fn apply(spec: &Spec, var: &dyn Fn(&str) -> Option<Value>, v: &mut OptionVector) -> bool {
    let mut complete = true;
    for &(col, source) in &spec.columns {
        let n = match source {
            Source::Lit(n) => Some(n),
            Source::Var { name, neg } => match var(name) {
                Some(Value::Number(n)) => Some(if neg { -n } else { n }),
                _ => None,
            },
            Source::Id(_) => None,
        };
        match n {
            Some(n) => v.columns[col] += n,
            None => complete = false,
        }
    }
    let id_of = |source: Source| match source {
        Source::Id(id) => Some(id),
        Source::Var { name, .. } => match var(name) {
            Some(Value::Id(id)) => Some(id),
            _ => None,
        },
        Source::Lit(_) => None,
    };
    for &(slot, source) in &spec.ids {
        match id_of(source) {
            Some(id) => v.ids[slot] = slot_id(slot, id),
            None => complete = false,
        }
    }
    if let Some(source) = spec.ench {
        match id_of(source).and_then(EnchantmentKind::from_id) {
            Some(kind) => v.ench = kind as u8 + 1,
            None => complete = false,
        }
    }
    complete
}

/// Slot `slot`'s vocab id of `id` (a card for the card slots, ...).
fn slot_id(slot: usize, id: &str) -> u16 {
    match ID_SLOT_NAMES[slot] {
        "card" | "curse" | "lost_card" => vocab::card_id(id),
        "relic" | "lost_relic" => vocab::relic_id(id),
        _ => vocab::potion_id(id),
    }
}

/// The relic `id` as an option: `relics=1`, its id, and its pickup effects ([`RELIC_SPECS`],
/// vars from its canonical ones); complete if it has a spec and every var a value.
fn relic_option(id: &str, v: &mut OptionVector) -> bool {
    v.columns[col("relics")] += 1;
    v.ids[slot("relic")] = vocab::relic_id(id);
    let Some(spec) = specs().relics.get(id) else {
        return false;
    };
    let kind = RelicKind::from_id(id);
    let var = |name: &str| {
        kind?
            .canonical_vars()
            .iter()
            .find(|v| v.name == name)?
            .value
            .map(|d| Value::Number(d.to_int_truncating()))
    };
    apply(spec, &var, v)
}

const fn col_at(name: &str) -> Option<usize> {
    let mut i = 0;
    while i < COLUMNS {
        if str_eq(COLUMN_NAMES[i], name) {
            return Some(i);
        }
        i += 1;
    }
    None
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn col(name: &str) -> usize {
    col_at(name).unwrap_or_else(|| panic!("no column {name}"))
}

fn slot(name: &str) -> usize {
    ID_SLOT_NAMES
        .iter()
        .position(|&s| s == name)
        .unwrap_or_else(|| panic!("no slot {name}"))
}

/// The id a text var names (a card, relic, potion or enchantment), if any.
const fn text_id(text: VarText) -> Option<&'static str> {
    match text {
        VarText::Card(id) | VarText::Relic(id) | VarText::Potion(id) | VarText::Enchantment(id) => {
            Some(id)
        }
        _ => None,
    }
}

/// The vector of event option `option` of `room` (module doc).
#[must_use]
pub fn event_option(room: &EventRoom, s: &RunState, option: &EventOption) -> OptionVector {
    let vars = room.vars(s);
    event_option_with(room, s, option, &vars)
}

fn event_option_with(
    room: &EventRoom,
    s: &RunState,
    option: &EventOption,
    vars: &[(&'static str, EventVar)],
) -> OptionVector {
    let mut v = OptionVector::default();
    let Some(action) = option.action else {
        // Locked: shown, never chosen; it does nothing.
        v.columns[0] = 1;
        return v;
    };
    let var = |name: &str| {
        vars.iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, value)| match *value {
                EventVar::Number(d) => Some(Value::Number(d.to_int_truncating())),
                EventVar::Text(t) => text_id(t).map(Value::Id),
            })
    };
    let known = match (room.kind, action.tag) {
        // `OptionFromChoice`: the doll's raw title as its key.
        (EventKind::DollRoom, "TAKE") => relic_option(DOLLS[action.arg], &mut v),
        // A relic on sale at its shown `Cost`.
        (EventKind::FakeMerchant, "BUY") => {
            let cost = match &room.data {
                EventData::FakeMerchant { relics } => relics.get(action.arg).map(|r| r.1),
                _ => None,
            };
            let relic = option.text_key.rsplit('.').next().unwrap_or_default();
            v.columns[col("relics")] = 1;
            v.ids[slot("relic")] = vocab::relic_id(relic);
            cost.is_some_and(|cost| {
                v.columns[col("gold")] = -cost;
                true
            })
        }
        // The potion in that belt slot is lost.
        (EventKind::TheFutureOfPotions, _) => {
            if let Some(Some(potion)) = s.potions.get(action.arg) {
                v.ids[slot("lost_potion")] = vocab::potion_id(potion);
            }
            event_spec(&option.text_key)
                .is_some_and(|(spec, known)| apply(spec, &var, &mut v) && known)
        }
        _ => match event_spec(&option.text_key) {
            Some((spec, known)) => match spec.relic {
                Some(relic) => relic_option(relic, &mut v),
                None => apply(spec, &var, &mut v) && known,
            },
            None => false,
        },
    };
    v.columns[0] = i32::from(known);
    v
}

/// `DollRoom._dolls`, in declaration order (the `TAKE` options' argument).
const DOLLS: [&str; 3] = ["DAUGHTER_OF_THE_WIND", "MR_STRUGGLES", "BING_BONG"];

/// The vector of ancient option `option` (module doc).
#[must_use]
pub fn ancient_option(option: AncientOption) -> OptionVector {
    let mut v = OptionVector::default();
    let known = match option {
        AncientOption::Relic { id, .. } => relic_option(id, &mut v),
        AncientOption::Locked(_) => true,
        AncientOption::Modifier(_) => false,
    };
    v.columns[0] = i32::from(known);
    v
}

/// Writes the section for the pending `decision` into `out` ([`EVENTS_BYTES`], zeroed first).
///
/// # Panics
/// If `out` is shorter than [`EVENTS_BYTES`].
pub fn write(s: &RunState, decision: &RunDecision<'_>, out: &mut [u8]) {
    let out = &mut out[..EVENTS_BYTES];
    out.fill(0);
    let vectors: Vec<OptionVector> = match decision {
        RunDecision::Event(room) => {
            let vars = room.vars(s);
            room.options
                .iter()
                .take(OPTIONS)
                .map(|o| event_option_with(room, s, o, &vars))
                .collect()
        }
        RunDecision::Ancient(room) => room
            .options
            .iter()
            .take(OPTIONS)
            .map(|&o| ancient_option(o))
            .collect(),
        _ => return,
    };
    for (v, chunk) in vectors.iter().zip(out.as_chunks_mut::<OPTION_BYTES>().0) {
        v.write(chunk);
    }
}

/// Every spec's problems with its ids and vars, for the tests.
///
/// `(key, problems)` for each spec that names an id
/// the engine lacks, a curse slot holding a non-curse (or a card slot a curse), or a var its
/// event (relic) does not declare.
#[must_use]
pub fn check_specs() -> Vec<(&'static str, String)> {
    let mut bad = Vec::new();
    let mut bad_extra = Vec::new();
    let event_of = |key: &str| {
        let id = key.split('.').next().unwrap_or_default();
        EventKind::ALL.into_iter().find(|k| k.id() == id)
    };
    let mut check = |key: &'static str, spec: &Spec, vars: &dyn Fn(&str) -> Option<bool>| {
        let problems = spec_problems(spec, vars);
        if !problems.is_empty() {
            bad.push((key, problems.join(", ")));
        }
    };
    let mut add = |key: &'static str, problem: &str| bad_extra.push((key, problem.to_owned()));
    for &(key, spec) in EVENT_SPECS.iter().chain(GAPS) {
        let Ok(parsed) = parse(spec) else {
            add(key, "does not parse");
            continue;
        };
        let Some(event) = event_of(key) else {
            // `RelicTrader`'s `PROCEED` names no event and no var.
            if parsed
                .columns
                .iter()
                .all(|(_, s)| matches!(s, Source::Lit(_)))
                && parsed.ids.is_empty()
            {
                continue;
            }
            add(key, "no such event");
            continue;
        };
        // Canonical vars, plus the ones `EventRoom::vars` sets: a var with a text is a text
        // var; the set ones are numbers unless they name content.
        let vars = |name: &str| -> Option<bool> {
            if let Some(v) = event.canonical_vars().iter().find(|v| v.name == name) {
                return Some(v.text.is_some() || DYNAMIC_TEXT_VARS.contains(&name));
            }
            if DYNAMIC_TEXT_VARS.contains(&name) {
                return Some(true);
            }
            DYNAMIC_NUMBER_VARS.contains(&name).then_some(false)
        };
        check(key, &parsed, &vars);
    }
    for &(id, spec) in RELIC_SPECS {
        let Ok(parsed) = parse(spec) else {
            add(id, "does not parse");
            continue;
        };
        let Some(kind) = RelicKind::from_id(id) else {
            add(id, "no such relic");
            continue;
        };
        let vars = |name: &str| {
            kind.canonical_vars()
                .iter()
                .find(|v| v.name == name)
                .map(|v| v.value.is_none())
        };
        check(id, &parsed, &vars);
    }
    bad.extend(bad_extra);
    bad
}

/// What is wrong with `spec`'s ids and vars (`vars`: `Some(true)` a text var, `Some(false)` a
/// number var, `None` none).
fn spec_problems(spec: &Spec, vars: &dyn Fn(&str) -> Option<bool>) -> Vec<String> {
    {
        let mut problems = Vec::new();
        let mut id_ok = |slot: usize, source: Source| match source {
            Source::Id(id) => {
                let name = ID_SLOT_NAMES[slot];
                let ok = match name {
                    "card" | "lost_card" => {
                        CardKind::from_id(id).is_some_and(|k| k.card_type() != CardType::Curse)
                    }
                    "curse" => {
                        CardKind::from_id(id).is_some_and(|k| k.card_type() == CardType::Curse)
                    }
                    "relic" | "lost_relic" => {
                        RelicKind::from_id(id).is_some() || UNPORTED_RELICS.contains(&id)
                    }
                    _ => PotionKind::from_id(id).is_some(),
                };
                if !ok {
                    problems.push(format!("{name}={id}"));
                }
            }
            Source::Var { name, .. } => {
                if vars(name) != Some(true) {
                    problems.push(format!("${name} is not a text var"));
                }
            }
            Source::Lit(_) => problems.push("a number in an id slot".to_owned()),
        };
        for &(slot, source) in &spec.ids {
            id_ok(slot, source);
        }
        match spec.ench {
            Some(Source::Id(id)) if EnchantmentKind::from_id(id).is_none() => {
                problems.push(format!("ench={id}"));
            }
            Some(Source::Var { name, .. }) if vars(name) != Some(true) => {
                problems.push(format!("${name} is not a text var"));
            }
            _ => {}
        }
        for &(_, source) in &spec.columns {
            if let Source::Var { name, .. } = source
                && vars(name) != Some(false)
            {
                problems.push(format!("${name} is not a number var"));
            }
        }
        if let Some(relic) = spec.relic
            && !specs().relics.contains_key(relic)
        {
            problems.push(format!("@{relic} has no relic spec"));
        }
        problems
    }
}

/// Relics an option names that the engine (and the vocab) lack: their slot holds `<unk>`.
const UNPORTED_RELICS: &[&str] = &["PRISMATIC_SHARD"];

/// The vars [`EventRoom::vars`] adds beyond the canonical ones (`display.rs`), numbers.
const DYNAMIC_NUMBER_VARS: &[&str] = &["Prize1", "Prize2", "Prize3"];
/// The same, naming content (a card, relic, potion).
const DYNAMIC_TEXT_VARS: &[&str] = &[
    "RandomCard",
    "RandomRelic",
    "DrinkRandomPotion",
    "TopRelicOwned",
    "TopRelicNew",
    "MiddleRelicOwned",
    "MiddleRelicNew",
    "BottomRelicOwned",
    "BottomRelicNew",
];
