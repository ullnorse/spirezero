//! The kernel's content tables, built once from sts2-core's content ([`Kernel::new`]).
//!
//! What is data in sts2-core is read from it: card costs, types, targets, keywords and var values
//! (per upgrade level), the shuffle order of cards, power types and flags, monster HP ranges, move
//! state machines (states, follow-ups, branches, weights, repeat rules, cooldowns) and encounter
//! slots. What is code in sts2-core (a card's `OnPlay`, a monster's `PerformMove`) is declared here
//! as a short list of [`CardOp`]s or [`MoveOp`]s that one interpreter (`engine.rs`) runs; the few
//! irregular behaviours (Plow, Illusion, Infested, the afflicting powers) are hand-written there.
//! The differential test (`tests/differential.rs`) is what checks the declared lists.

use std::sync::OnceLock;

use sts2_core::combat::state::Card;
use sts2_core::combat::{CardId, Encounter};
use sts2_core::content::cards::{CardKeyword, CardKind, CardType, TargetType, Var};
use sts2_core::content::monsters::machine::{MoveTable, Repeat, StateDef};
use sts2_core::content::monsters::{EyeWithTeeth, MonsterKind, Wriggler};
use sts2_core::content::powers::{PowerKind, PowerType};
use sts2_core::run::rooms::{OVERGROWTH, UNDERDOCKS};

/// A power the kernel supports, in [`POWERS`] order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum P {
    #[default]
    Strength,
    Vulnerable,
    Weak,
    Frail,
    Artifact,
    Slow,
    Territorial,
    Plow,
    Ringing,
    Slippery,
    Shrink,
    Infested,
    Constrict,
    Minion,
    Tangled,
    Illusion,
    Ritual,
    Plating,
    Thorns,
    Ravenous,
    Suck,
    Skittish,
    HardenedShell,
    Shriek,
    Vigor,
    Asleep,
    Dexterity,
    Intangible,
}

/// Every supported power with its sts2-core kind, in [`P`] order.
pub const POWERS: [(P, PowerKind); 28] = [
    (P::Strength, PowerKind::Strength),
    (P::Vulnerable, PowerKind::Vulnerable),
    (P::Weak, PowerKind::Weak),
    (P::Frail, PowerKind::Frail),
    (P::Artifact, PowerKind::Artifact),
    (P::Slow, PowerKind::Slow),
    (P::Territorial, PowerKind::Territorial),
    (P::Plow, PowerKind::Plow),
    (P::Ringing, PowerKind::Ringing),
    (P::Slippery, PowerKind::Slippery),
    (P::Shrink, PowerKind::Shrink),
    (P::Infested, PowerKind::Infested),
    (P::Constrict, PowerKind::Constrict),
    (P::Minion, PowerKind::Minion),
    (P::Tangled, PowerKind::Tangled),
    (P::Illusion, PowerKind::Illusion),
    (P::Ritual, PowerKind::Ritual),
    (P::Plating, PowerKind::Plating),
    (P::Thorns, PowerKind::Thorns),
    (P::Ravenous, PowerKind::Ravenous),
    (P::Suck, PowerKind::Suck),
    (P::Skittish, PowerKind::Skittish),
    (P::HardenedShell, PowerKind::HardenedShell),
    (P::Shriek, PowerKind::Shriek),
    (P::Vigor, PowerKind::Vigor),
    (P::Asleep, PowerKind::Asleep),
    (P::Dexterity, PowerKind::Dexterity),
    (P::Intangible, PowerKind::Intangible),
];

impl P {
    /// The bit of this power in a presence mask.
    #[must_use]
    pub const fn bit(self) -> u32 {
        1 << self as u8
    }

    /// The sts2-core kind.
    #[must_use]
    pub const fn core(self) -> PowerKind {
        POWERS[self as usize].1
    }

    /// The kernel power for an sts2-core kind, if supported.
    #[must_use]
    pub fn from_core(kind: PowerKind) -> Option<Self> {
        POWERS.iter().find(|(_, k)| *k == kind).map(|&(p, _)| p)
    }
}

/// A power's static data, read from sts2-core's `PowerModel`.
#[derive(Clone, Copy, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent `PowerModel` property"
)]
pub struct PowerInfo {
    pub debuff: bool,
    pub allows_negative: bool,
    /// `ShouldPowerBeRemovedAfterOwnerDeath`.
    pub removed_on_owner_death: bool,
    /// `OwnerIsSecondaryEnemy`.
    pub secondary: bool,
    /// `ITemporaryPower`.
    pub temporary: bool,
}

/// A card's effect, in `OnPlay` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardOp {
    /// `DamageCmd.Attack(damage).FromCard(this).Targeting(target)`.
    Attack(i32),
    /// `CreatureCmd.GainBlock(owner, block, Move, play)`.
    Block(i32),
    /// `PowerCmd.Apply<T>(target, amount, owner, this)`.
    ApplyTarget(P, i32),
    /// `CardPileCmd.Draw(count)`.
    Draw(i32),
    /// `CardCmd.Discard(FromHandForDiscard(.., 1))`: discard exactly one hand card, chosen.
    DiscardOne,
}

/// The supported cards' affliction (`Card::affliction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Affliction {
    #[default]
    None,
    Ringing,
    Entangled,
}

/// A card kind at an upgrade level.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent `CardModel` property"
)]
pub struct CardSpec {
    pub kind: CardKind,
    pub upgrade: u8,
    /// `EnergyCost` base: negative for an unplayable card.
    pub cost: i32,
    pub card_type: CardType,
    /// `AnyEnemy`: played on a living enemy; otherwise untargeted.
    pub targeted: bool,
    pub unplayable: bool,
    pub exhaust: bool,
    pub ethereal: bool,
    /// Position in `CardModel.CompareTo` order (entry, then upgrade level): the shuffle's sort.
    pub rank: u16,
    pub ops: Vec<CardOp>,
    /// `OnTurnEndInHand`: damage to the owner.
    pub turn_end: Option<TurnEnd>,
}

/// A card's `OnTurnEndInHand`: `amount` damage to its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnEnd {
    pub amount: i32,
    /// `Infection`: the owner deals it (`Unpowered | Move`). `Beckon`: no dealer, and unblockable
    /// (`Unblockable | Unpowered | Move`).
    pub unblockable: bool,
}

/// An ascension-dependent number (`AscensionHelper.GetValueIfAscension`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Asc {
    pub level: u8,
    pub then: i32,
    pub otherwise: i32,
}

impl Asc {
    /// The same at every ascension.
    #[must_use]
    pub const fn flat(value: i32) -> Self {
        Self {
            level: 0,
            then: value,
            otherwise: value,
        }
    }

    /// `then` from `level` on, else `otherwise`.
    #[must_use]
    pub const fn at(level: u8, then: i32, otherwise: i32) -> Self {
        Self {
            level,
            then,
            otherwise,
        }
    }

    /// The value at `ascension`.
    #[must_use]
    pub const fn get(self, ascension: u8) -> i32 {
        if ascension >= self.level {
            self.then
        } else {
            self.otherwise
        }
    }
}

/// `AscensionLevel.ToughEnemies`.
const TOUGH: u8 = 8;
/// `AscensionLevel.DeadlyEnemies`.
const DEADLY: u8 = 9;

/// One step of a monster move (or of its `AfterAddedToRoom`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveOp {
    /// `DamageCmd.Attack(damage).WithHitCount(hits).FromMonster(this)` on the players.
    Attack { damage: Asc, hits: Asc },
    /// `CreatureCmd.GainBlock(this, amount, Move)`.
    Block(Asc),
    /// `PowerCmd.Apply<T>(this, amount, this)`.
    ApplySelf(P, Asc),
    /// `PowerCmd.Apply<T>(PlayerCreatures, amount, this)`.
    ApplyPlayers(P, Asc),
    /// `CardPileCmd.AddToCombatAndPreview<T>(PlayerCreatures, Discard, count)`. With `direct`,
    /// `CombatState.CreateCard` and `AddGeneratedCardToCombat` instead (`SoulFysh`: no dead-player
    /// check), and with `random_draw` into the draw pile at a random position.
    AddCards {
        card: u8,
        count: u8,
        direct: bool,
        random_draw: bool,
    },
    /// `CreatureCmd.Add<T>(slot)`: a monster of kernel kind `monster`, in the encounter slot named
    /// `slot`.
    Summon { monster: u8, slot: &'static str },
}

/// A `ConditionalBranchState` condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    /// `Nibbit.IsFront`, `Toadpole.IsFront` (the creature's `aux`).
    Front,
    /// `!IsFront`.
    NotFront,
    /// The creature sits in the encounter slot of that name (`Wriggler`).
    Slot(&'static str),
    /// The creature has (or lacks) a power of that kind (`LagavulinMatriarch`'s sleep).
    Has(P),
    Lacks(P),
}

/// One weighted branch of a `RandomBranchState`.
#[derive(Clone, Copy, Debug)]
pub struct BranchSpec {
    pub state: u8,
    pub repeat: Repeat,
    pub cooldown: u8,
    pub weight: f32,
}

/// A `MonsterState`.
#[derive(Clone, Debug)]
pub enum StateSpec {
    Move { ops: Vec<MoveOp>, follow_up: u8 },
    Random { branches: Vec<BranchSpec> },
    Conditional { branches: Vec<(u8, Cond)> },
}

impl StateSpec {
    /// `IsMove`.
    #[must_use]
    pub const fn is_move(&self) -> bool {
        matches!(self, Self::Move { .. })
    }
}

/// A monster kind.
#[derive(Clone, Debug)]
pub struct MonsterSpec {
    /// `ModelId.Entry`.
    pub id: &'static str,
    pub states: Vec<StateSpec>,
    /// The machine's initial state when the kernel creates this monster (a summon).
    pub initial: u8,
    /// `MinInitialHp` and `MaxInitialHp` per ascension 0..=20, for summons.
    pub hp: Vec<(i32, i32)>,
    pub after_added: Vec<MoveOp>,
    /// The state a power-driven stun leads to: `CeremonialBeast.BeastCryState` (`PlowPower`),
    /// `TerrorEel.TerrorState` (`ShriekPower`), `LagavulinMatriarch` Slash (`AsleepPower`).
    pub stun_follow_up: Option<u8>,
}

/// An encounter's slots (`EncounterModel.Slots`).
#[derive(Clone, Debug)]
pub struct EncounterSpec {
    pub encounter: Encounter,
    pub slots: &'static [&'static str],
}

/// The relics the kernel supports, as bits of [`crate::Fight::relics`].
pub mod relic {
    /// `BurningBlood`: heal 6 after a won combat.
    pub const BURNING_BLOOD: u8 = 1;
    /// `RingOfTheSnake`: 2 more cards in the first hand draw.
    pub const RING_OF_THE_SNAKE: u8 = 2;
}

/// Every content table.
#[derive(Debug)]
pub struct Kernel {
    pub cards: Vec<CardSpec>,
    pub powers: [PowerInfo; POWERS.len()],
    pub monsters: Vec<MonsterSpec>,
    pub encounters: Vec<EncounterSpec>,
}

/// The cards the kernel plays, with their effect shapes.
const CARDS: &[(CardKind, &[Shape])] = &[
    (CardKind::StrikeIronclad, &[Shape::Attack]),
    (CardKind::DefendIronclad, &[Shape::Block]),
    (
        CardKind::Bash,
        &[Shape::Attack, Shape::Apply(P::Vulnerable)],
    ),
    (CardKind::StrikeSilent, &[Shape::Attack]),
    (CardKind::DefendSilent, &[Shape::Block]),
    (
        CardKind::Neutralize,
        &[Shape::Attack, Shape::Apply(P::Weak)],
    ),
    (CardKind::Survivor, &[Shape::Block, Shape::DiscardOne]),
    (CardKind::Slimed, &[Shape::Draw]),
    (CardKind::Infection, &[]),
    (CardKind::Wound, &[]),
    (CardKind::Dazed, &[]),
    (CardKind::AscendersBane, &[]),
    (CardKind::Beckon, &[]),
];

/// The shape of a card op; [`Kernel::new`] reads its number from the card's vars.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Attack,
    Block,
    Apply(P),
    Draw,
    DiscardOne,
}

/// A monster's move ops by move id.
type MoveList = Vec<(&'static str, Vec<MoveOp>)>;

/// The monsters' move ops, by move id. Numbers are `PerformMove`'s (`content/monsters/*.rs`).
#[allow(
    clippy::too_many_lines,
    reason = "one table entry per monster, like the content files it mirrors"
)]
fn monster_moves(id: &str) -> Option<(MoveList, Vec<MoveOp>)> {
    use MoveOp::{AddCards, ApplyPlayers, ApplySelf, Attack, Block, Summon};
    let hit = |level, then, otherwise| Attack {
        damage: Asc::at(level, then, otherwise),
        hits: Asc::flat(1),
    };
    let hits = |level, then, otherwise, n| Attack {
        damage: Asc::at(level, then, otherwise),
        hits: Asc::flat(n),
    };
    let flat = Asc::flat;
    let c = |kind| card_index(kind);
    let moves: MoveList;
    let mut added = Vec::new();
    match id {
        "FUZZY_WURM_CRAWLER" => {
            moves = vec![
                ("FIRST_ACID_GOOP", vec![hit(DEADLY, 6, 4)]),
                ("INHALE", vec![ApplySelf(P::Strength, flat(7))]),
                ("ACID_GOOP", vec![hit(DEADLY, 6, 4)]),
            ];
        }
        "LEAF_SLIME_M" => {
            moves = vec![
                ("CLUMP_SHOT", vec![hit(DEADLY, 9, 8)]),
                (
                    "STICKY_SHOT",
                    vec![AddCards {
                        card: c(CardKind::Slimed),
                        count: 2,
                        direct: false,
                        random_draw: false,
                    }],
                ),
            ];
        }
        "LEAF_SLIME_S" => {
            moves = vec![
                ("TACKLE_MOVE", vec![hit(DEADLY, 4, 3)]),
                (
                    "GOOP_MOVE",
                    vec![AddCards {
                        card: c(CardKind::Slimed),
                        count: 1,
                        direct: false,
                        random_draw: false,
                    }],
                ),
            ];
        }
        "TWIG_SLIME_M" => {
            moves = vec![
                ("POKEY_POUNCE_MOVE", vec![hit(DEADLY, 12, 11)]),
                (
                    "STICKY_SHOT_MOVE",
                    vec![AddCards {
                        card: c(CardKind::Slimed),
                        count: 1,
                        direct: false,
                        random_draw: false,
                    }],
                ),
            ];
        }
        "TWIG_SLIME_S" => moves = vec![("TACKLE_MOVE", vec![hit(DEADLY, 5, 4)])],
        "NIBBIT" => {
            moves = vec![
                ("BUTT_MOVE", vec![hit(DEADLY, 13, 12)]),
                (
                    "SLICE_MOVE",
                    vec![hit(DEADLY, 7, 6), Block(Asc::at(TOUGH, 6, 5))],
                ),
                (
                    "HISS_MOVE",
                    vec![ApplySelf(P::Strength, Asc::at(DEADLY, 3, 2))],
                ),
            ];
        }
        "SHRINKER_BEETLE" => {
            moves = vec![
                ("SHRINKER_MOVE", vec![ApplyPlayers(P::Shrink, flat(-1))]),
                ("CHOMP_MOVE", vec![hit(DEADLY, 8, 7)]),
                ("STOMP_MOVE", vec![hit(DEADLY, 14, 13)]),
            ];
        }
        "BYGONE_EFFIGY" => {
            moves = vec![
                ("SLEEP_MOVE", vec![]),
                ("WAKE_MOVE", vec![ApplySelf(P::Strength, flat(10))]),
                ("SLASHES_MOVE", vec![hit(DEADLY, 15, 13)]),
            ];
            added = vec![ApplySelf(P::Slow, flat(1))];
        }
        "BYRDONIS" => {
            moves = vec![
                ("SWOOP_MOVE", vec![hit(DEADLY, 19, 17)]),
                ("PECK_MOVE", vec![hits(DEADLY, 4, 3, 3)]),
            ];
            added = vec![ApplySelf(P::Territorial, flat(1))];
        }
        "CEREMONIAL_BEAST" => {
            moves = vec![
                (
                    "PLOW_MOVE",
                    vec![hit(DEADLY, 20, 18), ApplySelf(P::Strength, flat(2))],
                ),
                (
                    "STAMP_MOVE",
                    vec![ApplySelf(P::Plow, Asc::at(DEADLY, 160, 150))],
                ),
                ("BEAST_CRY_MOVE", vec![ApplyPlayers(P::Ringing, flat(1))]),
                ("STOMP_MOVE", vec![hit(DEADLY, 17, 15)]),
                (
                    "CRUSH_MOVE",
                    vec![
                        hit(DEADLY, 19, 17),
                        ApplySelf(P::Strength, Asc::at(DEADLY, 4, 3)),
                    ],
                ),
            ];
        }
        "CUBEX_CONSTRUCT" => {
            let blast = vec![hit(DEADLY, 8, 7), ApplySelf(P::Strength, flat(2))];
            moves = vec![
                ("CHARGE_UP_MOVE", vec![ApplySelf(P::Strength, flat(2))]),
                ("REPEATER_BLAST_MOVE", blast.clone()),
                ("REPEATER_BLAST_MOVE_2", blast),
                ("EXPEL_MOVE", vec![hits(DEADLY, 6, 5, 2)]),
            ];
            added = vec![Block(flat(13)), ApplySelf(P::Artifact, flat(1))];
        }
        "FLYCONID" => {
            moves = vec![
                (
                    "VULNERABLE_SPORES_MOVE",
                    vec![ApplyPlayers(P::Vulnerable, flat(2))],
                ),
                (
                    "FRAIL_SPORES_MOVE",
                    vec![hit(DEADLY, 9, 8), ApplyPlayers(P::Frail, flat(2))],
                ),
                ("SMASH_MOVE", vec![hit(DEADLY, 12, 11)]),
            ];
        }
        "FOGMOG" => {
            let swipe = vec![hit(DEADLY, 9, 8), ApplySelf(P::Strength, flat(1))];
            moves = vec![
                (
                    "ILLUSION_MOVE",
                    vec![Summon {
                        monster: 0,
                        slot: "illusion",
                    }],
                ),
                ("SWIPE_MOVE", swipe.clone()),
                ("SWIPE_RANDOM_MOVE", swipe),
                ("HEADBUTT_MOVE", vec![hit(DEADLY, 16, 14)]),
            ];
        }
        "EYE_WITH_TEETH" => {
            moves = vec![(
                "DISTRACT_MOVE",
                vec![AddCards {
                    card: c(CardKind::Dazed),
                    count: 3,
                    direct: false,
                    random_draw: false,
                }],
            )];
            added = vec![ApplySelf(P::Illusion, flat(1))];
        }
        "INKLET" => {
            moves = vec![
                ("JAB_MOVE", vec![hit(DEADLY, 4, 3)]),
                ("PIERCING_GAZE_MOVE", vec![hit(DEADLY, 11, 10)]),
                ("WHIRLWIND_MOVE", vec![hits(DEADLY, 3, 2, 3)]),
            ];
            added = vec![ApplySelf(P::Slippery, flat(1))];
        }
        "MAWLER" => {
            moves = vec![
                ("RIP_AND_TEAR_MOVE", vec![hit(DEADLY, 16, 14)]),
                ("ROAR_MOVE", vec![ApplyPlayers(P::Vulnerable, flat(3))]),
                ("CLAW_MOVE", vec![hits(DEADLY, 5, 4, 2)]),
            ];
        }
        "PHROG_PARASITE" => {
            moves = vec![
                (
                    "INFECT_MOVE",
                    vec![AddCards {
                        card: c(CardKind::Infection),
                        count: 3,
                        direct: false,
                        random_draw: false,
                    }],
                ),
                ("LASH_MOVE", vec![hits(DEADLY, 5, 4, 4)]),
            ];
            added = vec![ApplySelf(P::Infested, flat(4))];
        }
        "WRIGGLER" => {
            moves = vec![
                ("SPAWNED_MOVE", vec![]),
                ("NASTY_BITE_MOVE", vec![hit(DEADLY, 7, 6)]),
                (
                    "WRIGGLE_MOVE",
                    vec![
                        AddCards {
                            card: c(CardKind::Infection),
                            count: 1,
                            direct: false,
                            random_draw: false,
                        },
                        ApplySelf(P::Strength, flat(2)),
                    ],
                ),
            ];
        }
        "ASSASSIN_RUBY_RAIDER" => moves = vec![("KILLSHOT_MOVE", vec![hit(DEADLY, 11, 10)])],
        "AXE_RUBY_RAIDER" => {
            let swing = vec![hit(DEADLY, 6, 5), Block(Asc::at(DEADLY, 6, 5))];
            moves = vec![
                ("SWING_1", swing.clone()),
                ("SWING_2", swing),
                ("BIG_SWING", vec![hit(DEADLY, 13, 12)]),
            ];
        }
        "BRUTE_RUBY_RAIDER" => {
            moves = vec![
                ("BEAT_MOVE", vec![hit(DEADLY, 8, 7)]),
                ("ROAR_MOVE", vec![ApplySelf(P::Strength, flat(3))]),
            ];
        }
        "CROSSBOW_RUBY_RAIDER" => {
            moves = vec![
                ("RELOAD_MOVE", vec![Block(flat(3))]),
                ("FIRE_MOVE", vec![hit(DEADLY, 16, 14)]),
            ];
        }
        "TRACKER_RUBY_RAIDER" => {
            moves = vec![
                ("TRACK_MOVE", vec![ApplyPlayers(P::Frail, flat(2))]),
                (
                    "HOUNDS_MOVE",
                    vec![Attack {
                        damage: flat(1),
                        hits: Asc::at(DEADLY, 9, 8),
                    }],
                ),
            ];
        }
        "SLITHERING_STRANGLER" => {
            moves = vec![
                ("THWACK", vec![hit(DEADLY, 8, 7), Block(flat(5))]),
                ("CONSTRICT", vec![ApplyPlayers(P::Constrict, flat(3))]),
                ("LASH", vec![hit(DEADLY, 13, 12)]),
            ];
        }
        "SNAPPING_JAXFRUIT" => {
            moves = vec![(
                "ENERGY_ORB_MOVE",
                vec![hit(DEADLY, 4, 3), ApplySelf(P::Strength, flat(2))],
            )];
        }
        "VINE_SHAMBLER" => {
            moves = vec![
                (
                    "GRASPING_VINES_MOVE",
                    vec![hit(DEADLY, 9, 8), ApplyPlayers(P::Tangled, flat(1))],
                ),
                ("SWIPE_MOVE", vec![hits(DEADLY, 7, 6, 2)]),
                ("CHOMP_MOVE", vec![hit(DEADLY, 18, 16)]),
            ];
        }
        "KIN_FOLLOWER" => {
            moves = vec![
                ("QUICK_SLASH_MOVE", vec![hit(0, 5, 5)]),
                ("BOOMERANG_MOVE", vec![hits(0, 2, 2, 2)]),
                (
                    "POWER_DANCE_MOVE",
                    vec![ApplySelf(P::Strength, Asc::at(DEADLY, 3, 2))],
                ),
            ];
            added = vec![ApplySelf(P::Minion, flat(1))];
        }
        "KIN_PRIEST" => {
            moves = vec![
                (
                    "ORB_OF_FRAILTY_MOVE",
                    vec![hit(DEADLY, 9, 8), ApplyPlayers(P::Frail, flat(1))],
                ),
                (
                    "ORB_OF_WEAKNESS_MOVE",
                    vec![hit(DEADLY, 9, 8), ApplyPlayers(P::Weak, flat(1))],
                ),
                ("BEAM_MOVE", vec![hits(0, 3, 3, 3)]),
                (
                    "RITUAL_MOVE",
                    vec![ApplySelf(P::Strength, Asc::at(DEADLY, 3, 2))],
                ),
            ];
        }
        "VANTOM" => {
            moves = vec![
                ("INK_BLOT_MOVE", vec![hit(DEADLY, 8, 7)]),
                ("INKY_LANCE_MOVE", vec![hits(DEADLY, 7, 6, 2)]),
                (
                    "DISMEMBER_MOVE",
                    vec![
                        hit(DEADLY, 30, 26),
                        AddCards {
                            card: c(CardKind::Wound),
                            count: 3,
                            direct: false,
                            random_draw: false,
                        },
                    ],
                ),
                ("PREPARE_MOVE", vec![ApplySelf(P::Strength, flat(2))]),
            ];
            added = vec![ApplySelf(P::Slippery, Asc::at(TOUGH, 9, 8))];
        }
        "SEAPUNK" => {
            moves = vec![
                ("SEA_KICK_MOVE", vec![hit(DEADLY, 13, 11)]),
                ("SPINNING_KICK_MOVE", vec![hits(0, 2, 2, 4)]),
                (
                    "BUBBLE_BURP_MOVE",
                    vec![
                        Block(Asc::at(TOUGH, 8, 7)),
                        ApplySelf(P::Strength, Asc::at(DEADLY, 2, 1)),
                    ],
                ),
            ];
        }
        "SLUDGE_SPINNER" => {
            moves = vec![
                (
                    "OIL_SPRAY_MOVE",
                    vec![hit(DEADLY, 9, 8), ApplyPlayers(P::Weak, flat(1))],
                ),
                ("SLAM_MOVE", vec![hit(DEADLY, 12, 11)]),
                (
                    "RAGE_MOVE",
                    vec![hit(DEADLY, 7, 6), ApplySelf(P::Strength, flat(3))],
                ),
            ];
        }
        "CALCIFIED_CULTIST" => {
            moves = vec![
                ("INCANTATION_MOVE", vec![ApplySelf(P::Ritual, flat(2))]),
                ("DARK_STRIKE_MOVE", vec![hit(DEADLY, 11, 9)]),
            ];
        }
        "DAMP_CULTIST" => {
            moves = vec![
                (
                    "INCANTATION_MOVE",
                    vec![ApplySelf(P::Ritual, Asc::at(DEADLY, 6, 5))],
                ),
                ("DARK_STRIKE_MOVE", vec![hit(DEADLY, 3, 1)]),
            ];
        }
        "SEWER_CLAM" => {
            moves = vec![
                ("PRESSURIZE_MOVE", vec![ApplySelf(P::Strength, flat(4))]),
                ("JET_MOVE", vec![hit(DEADLY, 11, 10)]),
            ];
            added = vec![ApplySelf(P::Plating, Asc::at(TOUGH, 9, 8))];
        }
        "PUNCH_CONSTRUCT" => {
            moves = vec![
                ("READY_MOVE", vec![Block(flat(10))]),
                (
                    "FAST_PUNCH_MOVE",
                    vec![hits(DEADLY, 6, 5, 2), ApplyPlayers(P::Frail, flat(1))],
                ),
                ("STRONG_PUNCH_MOVE", vec![hit(DEADLY, 16, 14)]),
            ];
        }
        "HAUNTED_SHIP" => {
            moves = vec![
                ("SWIPE_MOVE", vec![hit(DEADLY, 14, 13)]),
                ("STOMP_MOVE", vec![hits(DEADLY, 5, 4, 3)]),
                (
                    "HAUNT_MOVE",
                    vec![
                        ApplyPlayers(P::Weak, flat(3)),
                        AddCards {
                            card: c(CardKind::Dazed),
                            count: 5,
                            direct: false,
                            random_draw: false,
                        },
                    ],
                ),
            ];
        }
        "TOADPOLE" => {
            moves = vec![
                ("SPIKEN_MOVE", vec![ApplySelf(P::Thorns, flat(2))]),
                (
                    "SPIKE_SPIT_MOVE",
                    vec![ApplySelf(P::Thorns, flat(-2)), hits(DEADLY, 4, 3, 3)],
                ),
                ("WHIRL_MOVE", vec![hit(DEADLY, 8, 7)]),
            ];
        }
        "CORPSE_SLUG" => {
            moves = vec![
                ("WHIP_SLAP_MOVE", vec![hits(0, 3, 3, 2)]),
                ("GLOMP_MOVE", vec![hit(DEADLY, 9, 8)]),
                ("GOOP_MOVE", vec![ApplyPlayers(P::Frail, flat(2))]),
            ];
            added = vec![ApplySelf(P::Ravenous, Asc::at(DEADLY, 5, 4))];
        }
        "FOSSIL_STALKER" => {
            moves = vec![
                (
                    "TACKLE_MOVE",
                    vec![hit(DEADLY, 11, 9), ApplyPlayers(P::Frail, flat(1))],
                ),
                ("LATCH_MOVE", vec![hit(DEADLY, 14, 12)]),
                ("LASH_MOVE", vec![hits(DEADLY, 4, 3, 2)]),
            ];
            added = vec![ApplySelf(P::Suck, flat(3))];
        }
        "PHANTASMAL_GARDENER" => {
            moves = vec![
                ("BITE_MOVE", vec![hit(0, 5, 5)]),
                ("LASH_MOVE", vec![hit(0, 7, 7)]),
                ("FLAIL_MOVE", vec![hits(0, 1, 1, 3)]),
                (
                    "ENLARGE_MOVE",
                    vec![ApplySelf(P::Strength, Asc::at(DEADLY, 3, 2))],
                ),
            ];
            added = vec![ApplySelf(P::Skittish, Asc::at(TOUGH, 7, 6))];
        }
        "SKULKING_COLONY" => {
            let zoom = vec![hit(DEADLY, 16, 14)];
            moves = vec![
                ("ZOOM_MOVE", zoom.clone()),
                ("ZOOM_MOVE_2", zoom),
                (
                    "INERTIA_MOVE",
                    vec![
                        hit(DEADLY, 11, 9),
                        ApplySelf(P::Strength, Asc::at(DEADLY, 4, 2)),
                    ],
                ),
                ("PIERCING_STABS_MOVE", vec![hits(DEADLY, 8, 7, 2)]),
            ];
            added = vec![ApplySelf(P::HardenedShell, flat(20))];
        }
        "TERROR_EEL" => {
            moves = vec![
                ("CRASH_MOVE", vec![hit(DEADLY, 18, 16)]),
                (
                    "THRASH_MOVE",
                    vec![hits(DEADLY, 4, 3, 3), ApplySelf(P::Vigor, flat(6))],
                ),
                ("STUN_MOVE", vec![]),
                ("TERROR_MOVE", vec![ApplyPlayers(P::Vulnerable, flat(99))]),
            ];
            added = vec![ApplySelf(P::Shriek, Asc::at(TOUGH, 75, 70))];
        }
        "LAGAVULIN_MATRIARCH" => {
            moves = vec![
                ("SLEEP_MOVE", vec![]),
                ("SLASH_MOVE", vec![hit(DEADLY, 21, 19)]),
                (
                    "SLASH2_MOVE",
                    vec![hit(DEADLY, 14, 12), Block(Asc::at(TOUGH, 14, 12))],
                ),
                (
                    "SOUL_SIPHON_MOVE",
                    vec![
                        ApplyPlayers(P::Strength, flat(-2)),
                        ApplyPlayers(P::Dexterity, flat(-2)),
                        ApplySelf(P::Strength, flat(2)),
                    ],
                ),
                ("DISEMBOWEL_MOVE", vec![hits(DEADLY, 10, 9, 2)]),
            ];
            added = vec![
                ApplySelf(P::Plating, flat(12)),
                ApplySelf(P::Asleep, flat(3)),
            ];
        }
        "SOUL_FYSH" => {
            let beckon = |random_draw| AddCards {
                card: c(CardKind::Beckon),
                count: 1,
                direct: true,
                random_draw,
            };
            moves = vec![
                ("BECKON_MOVE", vec![beckon(true), beckon(false)]),
                ("DE_GAS_MOVE", vec![hit(DEADLY, 18, 16)]),
                ("GAZE_MOVE", vec![hit(DEADLY, 8, 7), beckon(false)]),
                (
                    "SCREAM_MOVE",
                    vec![hit(DEADLY, 15, 13), ApplyPlayers(P::Vulnerable, flat(3))],
                ),
                ("FADE_MOVE", vec![ApplySelf(P::Intangible, flat(2))]),
            ];
        }
        _ => return None,
    }
    Some((moves, added))
}

/// The monster kinds the kernel supports, as sts2-core values (their move table and HP are read
/// from these; per-instance fields only pick the initial state, which summons fix here).
fn supported_monsters() -> Vec<MonsterKind> {
    use sts2_core::content::monsters::{
        AssassinRubyRaider, AxeRubyRaider, BruteRubyRaider, BygoneEffigy, Byrdonis,
        CalcifiedCultist, CeremonialBeast, CorpseSlug, CrossbowRubyRaider, CubexConstruct,
        DampCultist, Flyconid, Fogmog, FossilStalker, FuzzyWurmCrawler, HauntedShip, Inklet,
        KinFollower, KinPriest, LagavulinMatriarch, LeafSlimeM, LeafSlimeS, Mawler, Nibbit,
        PhantasmalGardener, PhrogParasite, PunchConstruct, Seapunk, SewerClam, ShrinkerBeetle,
        SkulkingColony, SlitheringStrangler, SludgeSpinner, SnappingJaxfruit, SoulFysh, TerrorEel,
        Toadpole, TrackerRubyRaider, TwigSlimeM, TwigSlimeS, Vantom, VineShambler,
    };
    // Kernel id 0 is `EyeWithTeeth` (`MoveOp::Summon` in Fogmog's table names it by index).
    vec![
        MonsterKind::EyeWithTeeth(EyeWithTeeth),
        MonsterKind::Wriggler(Wriggler {
            start_stunned: true,
        }),
        MonsterKind::FuzzyWurmCrawler(FuzzyWurmCrawler::new()),
        MonsterKind::LeafSlimeM(LeafSlimeM),
        MonsterKind::LeafSlimeS(LeafSlimeS),
        MonsterKind::TwigSlimeM(TwigSlimeM),
        MonsterKind::TwigSlimeS(TwigSlimeS),
        MonsterKind::Nibbit(Nibbit::default()),
        MonsterKind::ShrinkerBeetle(ShrinkerBeetle),
        MonsterKind::BygoneEffigy(BygoneEffigy),
        MonsterKind::Byrdonis(Byrdonis),
        MonsterKind::CeremonialBeast(CeremonialBeast),
        MonsterKind::CubexConstruct(CubexConstruct),
        MonsterKind::Flyconid(Flyconid),
        MonsterKind::Fogmog(Fogmog),
        MonsterKind::Inklet(Inklet::default()),
        MonsterKind::Mawler(Mawler),
        MonsterKind::PhrogParasite(PhrogParasite),
        MonsterKind::AssassinRubyRaider(AssassinRubyRaider),
        MonsterKind::AxeRubyRaider(AxeRubyRaider),
        MonsterKind::BruteRubyRaider(BruteRubyRaider),
        MonsterKind::CrossbowRubyRaider(CrossbowRubyRaider),
        MonsterKind::TrackerRubyRaider(TrackerRubyRaider),
        MonsterKind::SlitheringStrangler(SlitheringStrangler),
        MonsterKind::SnappingJaxfruit(SnappingJaxfruit),
        MonsterKind::VineShambler(VineShambler),
        MonsterKind::KinFollower(KinFollower::default()),
        MonsterKind::KinPriest(KinPriest),
        MonsterKind::Vantom(Vantom),
        MonsterKind::Seapunk(Seapunk),
        MonsterKind::SludgeSpinner(SludgeSpinner),
        MonsterKind::CalcifiedCultist(CalcifiedCultist),
        MonsterKind::DampCultist(DampCultist),
        MonsterKind::SewerClam(SewerClam),
        MonsterKind::PunchConstruct(PunchConstruct::default()),
        MonsterKind::HauntedShip(HauntedShip),
        MonsterKind::Toadpole(Toadpole::default()),
        MonsterKind::CorpseSlug(CorpseSlug::default()),
        MonsterKind::FossilStalker(FossilStalker),
        MonsterKind::PhantasmalGardener(PhantasmalGardener),
        MonsterKind::SkulkingColony(SkulkingColony),
        MonsterKind::TerrorEel(TerrorEel),
        MonsterKind::LagavulinMatriarch(LagavulinMatriarch),
        MonsterKind::SoulFysh(SoulFysh),
    ]
}

/// The kernel index of `kind` in [`CARDS`] order, upgrade level 0.
fn card_index(kind: CardKind) -> u8 {
    let i = CARDS
        .iter()
        .position(|(k, _)| *k == kind)
        .expect("a supported card");
    u8::try_from(2 * i).expect("few cards")
}

/// A var of a fresh `kind` card at `upgrade`, as an integer.
fn card_var(kind: CardKind, upgrade: u8, var: Var) -> i32 {
    Card::new(CardId(0), kind, upgrade)
        .var(var)
        .to_int_truncating()
}

impl Kernel {
    /// The shared tables (built on first use).
    pub fn get() -> &'static Self {
        static KERNEL: OnceLock<Kernel> = OnceLock::new();
        KERNEL.get_or_init(Self::new)
    }

    /// Builds every table from sts2-core.
    ///
    /// # Panics
    /// If a declared move list does not match the monster's sts2-core move table (a move id that
    /// is not a move state, or a move state with no declared list).
    #[must_use]
    pub fn new() -> Self {
        let cards = Self::card_specs();
        let powers = POWERS.map(|(_, k)| PowerInfo {
            debuff: k.power_type() == PowerType::Debuff,
            allows_negative: k.allows_negative(),
            removed_on_owner_death: k.should_be_removed_after_owner_death(),
            secondary: k.owner_is_secondary_enemy(),
            temporary: k.is_temporary(),
        });
        let monsters = supported_monsters()
            .into_iter()
            .map(Self::monster_spec)
            .collect();
        let encounters = OVERGROWTH
            .encounters
            .iter()
            .chain(UNDERDOCKS.encounters)
            .filter_map(|info| Encounter::from_entry(info.id))
            .map(|encounter| EncounterSpec {
                encounter,
                slots: encounter.slots(),
            })
            .collect();
        Self {
            cards,
            powers,
            monsters,
            encounters,
        }
    }

    /// Two specs per [`CARDS`] entry: upgrade levels 0 and 1 (index `2i + level`); the second is
    /// a copy of the first for a card that cannot be upgraded.
    fn card_specs() -> Vec<CardSpec> {
        let mut specs = Vec::new();
        for &(kind, shapes) in CARDS {
            for upgrade in 0..2 {
                let level = if kind.is_upgradable(0) { upgrade } else { 0 };
                let card = Card::new(CardId(0), kind, level);
                let keywords = card.keywords;
                let ops = shapes
                    .iter()
                    .map(|shape| match *shape {
                        Shape::Attack => CardOp::Attack(card_var(kind, level, Var::Damage)),
                        Shape::Block => CardOp::Block(card_var(kind, level, Var::Block)),
                        Shape::Apply(p) => {
                            CardOp::ApplyTarget(p, card_var(kind, level, Var::Power(p.core())))
                        }
                        Shape::Draw => CardOp::Draw(card_var(kind, level, Var::Cards)),
                        Shape::DiscardOne => CardOp::DiscardOne,
                    })
                    .collect();
                specs.push(CardSpec {
                    kind,
                    upgrade: level,
                    cost: card.energy_cost.base,
                    card_type: kind.card_type(),
                    targeted: kind.target() == TargetType::AnyEnemy,
                    unplayable: keywords.contains(CardKeyword::Unplayable),
                    exhaust: keywords.contains(CardKeyword::Exhaust),
                    ethereal: keywords.contains(CardKeyword::Ethereal),
                    rank: 0,
                    ops,
                    turn_end: match kind {
                        CardKind::Infection => Some(TurnEnd {
                            amount: card_var(kind, level, Var::Damage),
                            unblockable: false,
                        }),
                        CardKind::Beckon => Some(TurnEnd {
                            amount: card_var(kind, level, Var::HpLoss),
                            unblockable: true,
                        }),
                        _ => {
                            assert!(!kind.has_turn_end_in_hand_effect(), "{kind:?}");
                            None
                        }
                    },
                });
            }
        }
        // `CardModel.CompareTo`: entry (ordinal), then upgrade level.
        let mut order: Vec<usize> = (0..specs.len()).collect();
        order.sort_by(|&a, &b| {
            let (a, b) = (&specs[a], &specs[b]);
            a.kind
                .entry()
                .cmp(b.kind.entry())
                .then(a.upgrade.cmp(&b.upgrade))
        });
        for (rank, &i) in order.iter().enumerate() {
            specs[i].rank = u16::try_from(rank).expect("few cards");
        }
        // Equal cards (a non-upgradable card's two copies) share a rank.
        for i in 0..specs.len() {
            if i % 2 == 1 && specs[i].upgrade == specs[i - 1].upgrade {
                specs[i].rank = specs[i - 1].rank;
            }
        }
        specs
    }

    /// The spec of a supported monster: its sts2-core move table with the declared move lists.
    fn monster_spec(kind: MonsterKind) -> MonsterSpec {
        let id = kind.id();
        let (moves, after_added) =
            monster_moves(id).unwrap_or_else(|| panic!("{id}: no declared moves"));
        let table: &MoveTable = kind.moves();
        let index = |state: usize| u8::try_from(state).expect("few states");
        let states = table
            .states
            .iter()
            .map(|def| match *def {
                StateDef::Move {
                    id: name,
                    follow_up,
                } => {
                    let ops = moves
                        .iter()
                        .find(|(n, _)| *n == name)
                        .unwrap_or_else(|| panic!("{id}: move {name} has no declared ops"))
                        .1
                        .clone();
                    StateSpec::Move {
                        ops,
                        follow_up: index(follow_up),
                    }
                }
                StateDef::Random { id: name, branches } => StateSpec::Random {
                    branches: branches
                        .iter()
                        .map(|b| {
                            assert!(b.weight_fn.is_none(), "{id} {name}: dynamic weights");
                            assert!(
                                b.cooldown <= 3
                                    && !matches!(b.repeat, Repeat::CanRepeatXTimes(n) if n > 3),
                                "{id} {name}: the monster log keeps three states"
                            );
                            BranchSpec {
                                state: index(b.state),
                                repeat: b.repeat,
                                cooldown: u8::try_from(b.cooldown).expect("short cooldown"),
                                weight: b.weight,
                            }
                        })
                        .collect(),
                },
                StateDef::Conditional { id: name, branches } => StateSpec::Conditional {
                    branches: branches
                        .iter()
                        .enumerate()
                        .map(|(i, &(state, _))| (index(state), condition(id, name, i)))
                        .collect(),
                },
            })
            .collect::<Vec<_>>();
        assert!(states.len() <= 16, "{id}: the used-state mask holds 16");
        for (name, _) in &moves {
            assert!(
                table.states.iter().any(|s| s.id() == *name && s.is_move()),
                "{id}: {name} is not a move state"
            );
        }
        let stun_state = match id {
            "CEREMONIAL_BEAST" => Some("BEAST_CRY_MOVE"),
            "TERROR_EEL" => Some("TERROR_MOVE"),
            "LAGAVULIN_MATRIARCH" => Some("SLASH_MOVE"),
            _ => None,
        };
        let stun_follow_up = stun_state.map(|name| {
            index(
                table
                    .states
                    .iter()
                    .position(|s| s.id() == name)
                    .expect("the stun state is in the table"),
            )
        });
        if matches!(kind, MonsterKind::CeremonialBeast(_)) {
            assert_eq!(
                stun_follow_up,
                Some(index(
                    sts2_core::content::monsters::CeremonialBeast::BEAST_CRY
                ))
            );
        }
        MonsterSpec {
            id,
            states,
            initial: index(table.initial),
            hp: (0..=20)
                .map(|a| (kind.min_initial_hp(a), kind.max_initial_hp(a)))
                .collect(),
            after_added,
            stun_follow_up,
        }
    }

    /// The kernel monster index of an sts2-core monster `id`.
    #[must_use]
    pub fn monster_index(&self, id: &str) -> Option<u8> {
        self.monsters
            .iter()
            .position(|m| m.id == id)
            .and_then(|i| u8::try_from(i).ok())
    }

    /// The kernel index of an sts2-core encounter.
    #[must_use]
    pub fn encounter_index(&self, encounter: Encounter) -> Option<u8> {
        self.encounters
            .iter()
            .position(|e| e.encounter == encounter)
            .and_then(|i| u8::try_from(i).ok())
    }

    /// The card spec of `kind` at `upgrade`, if supported.
    #[must_use]
    pub fn card_spec(&self, kind: CardKind, upgrade: u8) -> Option<u8> {
        let i = CARDS.iter().position(|(k, _)| *k == kind)?;
        let spec = 2 * i + usize::from(upgrade.min(1));
        (self.cards[spec].upgrade == upgrade)
            .then(|| u8::try_from(spec).ok())
            .flatten()
    }

    /// `PowerModel.GetTypeForAmount`: whether `amount` of `p` is a debuff.
    #[must_use]
    pub const fn debuff_amount(&self, p: P, amount: i32) -> bool {
        let info = self.power(p);
        if info.allows_negative && amount < 0 {
            true
        } else if !info.allows_negative && info.debuff && amount < 0 {
            false
        } else {
            info.debuff
        }
    }

    /// The static data of `p`.
    #[must_use]
    pub const fn power(&self, p: P) -> PowerInfo {
        self.powers[p as usize]
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

/// The kernel form of the `i`th condition of state `state` of monster `id`.
fn condition(id: &str, state: &str, i: usize) -> Cond {
    match (id, state, i) {
        // The grouped table (`!IsFront`, then `IsFront`). The kernel never re-enters `INIT_MOVE`:
        // sts2-core rolls every starting Nibbit out of it before the first player turn.
        ("NIBBIT" | "TOADPOLE", "INIT_MOVE", 0) => Cond::NotFront,
        ("NIBBIT" | "TOADPOLE", "INIT_MOVE", 1) => Cond::Front,
        ("PHANTASMAL_GARDENER", "INIT_MOVE", 0) => Cond::Slot("first"),
        ("PHANTASMAL_GARDENER", "INIT_MOVE", 1) => Cond::Slot("second"),
        ("PHANTASMAL_GARDENER", "INIT_MOVE", 2) => Cond::Slot("third"),
        ("PHANTASMAL_GARDENER", "INIT_MOVE", 3) => Cond::Slot("fourth"),
        ("LAGAVULIN_MATRIARCH", "SLEEP_BRANCH", 0) => Cond::Has(P::Asleep),
        ("LAGAVULIN_MATRIARCH", "SLEEP_BRANCH", 1) => Cond::Lacks(P::Asleep),
        ("WRIGGLER", "INIT_MOVE", 0) => Cond::Slot("wriggler1"),
        ("WRIGGLER", "INIT_MOVE", 1) => Cond::Slot("wriggler2"),
        ("WRIGGLER", "INIT_MOVE", 2) => Cond::Slot("wriggler3"),
        ("WRIGGLER", "INIT_MOVE", 3) => Cond::Slot("wriggler4"),
        _ => panic!("{id} {state}: unknown condition {i}"),
    }
}
