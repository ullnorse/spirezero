//! The run-level environment (`docs/specs/rl-2x-plan.md`, stage 9): whole runs as episodes.
//!
//! [`RunEnvs`] owns `n` runs and flat, stable buffers like [`crate::Envs`]: the observations
//! (`n` × [`RUN_OBS_BYTES`], [`crate::run_obs`]), the masks (`n` × [`RUN_MASK_WORDS`] words over
//! the run space of [`crate::run_action`]) and, per env, `done`, `won`, how the last episode
//! ended ([`RunEnd`]) and its final floor. An episode is a run from `Run::new` on a seed of the
//! [`SeedSource`] to death, victory, unported content, a position no action of which the engine
//! accepts, or [`MAX_RUN_ACTIONS`].
//!
//! [`RunEnvs::step`] applies one index per env, then plays on by itself until the next
//! decision with more than one option in the space:
//! - a decision with exactly one option is taken automatically and counted
//!   ([`RunStepStats::auto`]);
//! - in a hybrid mode ([`CombatMode::Greedy`], [`CombatMode::Random`]) every fight is played by
//!   the env's [`Resolver`] (`resolved`), so the agent sees only run-level choices; in
//!   [`CombatMode::Agent`] it plays the fights through the combat space;
//! - a listed action the engine would refuse (`Run::apply` rejects some legal actions whose
//!   content is not ported; [`Run::accepts`] tells which) has no bit in the mask (`refused`);
//!   with no bit left the episode ends as [`RunEnd::Stuck`].
//!
//! Episodes are handed out in env order after the envs have stepped, so which env plays which
//! seed, and every trajectory, is the same for any [`RunEnvs::threads`].
//!
//! [`RunOptions`] (all off by default, when the envs behave as before to the byte) add what the
//! end-to-end learner needs (`docs/plans/necro-e2e.md`, WP1):
//! - `dedupe`: the mask keeps one index per action class ([`crate::classes`]), and
//!   [`RunEnvs::class_counts`] has each kept index's member count; a decision left with one
//!   class is taken automatically like a single option ([`RunStepStats::auto_class`]);
//! - `turn_cap`, `decision_cap`: a fight past `turn_cap` player turns ends the run as
//!   [`RunEnd::FightCapped`], `decision_cap` agent decisions as [`RunEnd::DecisionCapped`] (both
//!   losses, reported as caps);
//! - `events`: per env and step, the floors entered, bosses killed and a won run
//!   ([`RunEvents`], [`RunEnvs::events`]), for reward shaping;
//! - `priv_obs`: [`RunEnvs::priv_obs`], the privileged buffer for the value head
//!   ([`crate::priv_obs`]); the observation never reads it;
//! - `counters`: per step, the decisions whose observation saturated a slot block
//!   ([`RunStepStats::saturated`]) or whose legal list had an action the space can't express.
//!
//! [`RunEnvs::set_practice`] (off until called) adds practice starts ([`crate::practice`]):
//! normal runs capture snapshots before elite and boss fights into a pool, and envs
//! `0..slots` start reseeded snapshots when their episode ends. A practice episode takes no
//! serial of the [`SeedSource`]; [`RunEnvs::cells`] tells which episode each env plays, and it
//! ends as [`RunEnd::PracticeEnd`] at its first decision outside combat, whose observation, mask
//! and privileged buffer are in [`RunEnvs::final_obs`], [`RunEnvs::final_masks`] and
//! [`RunEnvs::final_priv`] for the value bootstrap.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use sts2_core::Character;
use sts2_core::combat::{Action, DecisionRequest};
use sts2_core::run::{Act1, Room, RoomType, Run, RunAction, RunDecision, UnlockState};

use crate::action::{self, ACTIONS, OPTION_SLOTS};
use crate::classes::{Dedupe, DedupeScratch, record_mismatch, run_key_with};
use crate::human::{Gym, GymConfig, GymTicket, HumanStart};
use crate::obs::RECORD_FIELDS;
use crate::obs_v2::{PLAN_AT, PLAN_BYTES, V2_AT};
use crate::plan_column::{OBS_DEPTH, big_fight, write_plan_at};
use crate::practice::{self, Cell, Pool, PracticeConfig, PracticeStart, SegmentEnd, Snapshot};
use crate::priv_obs::{PRIV_BYTES, PrivObserver};
use crate::resolve::{CombatMode, Resolver};
use crate::run_action::{
    RUN_ACTIONS, RUN_MASK_WORDS, RunMask, check_run_mask, discard_offers, run_index_of,
};
use crate::run_obs::{DECK, MAP_POINTS, RELICS, RUN_OBS_BYTES, RunObserver};
use crate::seeds::SeedSource;
use crate::step_timing;

/// Engine actions a run may take (the agent's, the automatic ones and the resolver's) before it
/// ends as [`RunEnd::Capped`].
pub const MAX_RUN_ACTIONS: u64 = 60_000;

/// An action that leaves its env as it is for one [`RunEnvs::step`]: no action, no events.
///
/// With [`RunEnvs::restart`] it lets a learner replay each env's run to where a checkpoint
/// left it, envs of shorter histories holding.
mod search;
pub use search::{Requests, Search, SearchConfig, SearchResult, SearchStats, World};

pub const HOLD: u16 = u16::MAX;

/// How an episode ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RunEnd {
    /// Not ended.
    #[default]
    None = 0,
    /// The player died.
    Died = 1,
    /// The run was won.
    Victory = 2,
    /// The run met content the engine does not port (a fight stopped, or `Run::new` failed).
    Unsupported = 3,
    /// No option of the position is accepted.
    Stuck = 4,
    /// [`MAX_RUN_ACTIONS`] were taken.
    Capped = 5,
    /// A fight went past [`RunOptions::turn_cap`] player turns (a loss).
    FightCapped = 6,
    /// [`RunOptions::decision_cap`] agent decisions were taken (a loss).
    DecisionCapped = 7,
    /// A practice episode reached its first decision outside combat after its fight (a
    /// truncation, not a loss: the position goes on, in [`RunEnvs::final_obs`]).
    PracticeEnd = 8,
    /// A gym episode ([`crate::human`]) reached its first decision outside combat after its
    /// fight: terminal (the run around the fight is not the agent's; nothing is bootstrapped).
    GymEnd = 9,
}

impl RunEnd {
    /// Whether the episode was cut off by a cap ([`RunEnd::Capped`], [`RunEnd::FightCapped`],
    /// [`RunEnd::DecisionCapped`]).
    #[must_use]
    pub const fn is_cap(self) -> bool {
        matches!(
            self,
            Self::Capped | Self::FightCapped | Self::DecisionCapped
        )
    }
}

/// The end-to-end learner's env options (see the module docs); [`RunOptions::default`] is all
/// off, [`RunOptions::E2E`] the plan's settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent on/off switches, one per FFI config field"
)]
pub struct RunOptions {
    /// Merge the mask into action classes, and take a decision with one class automatically.
    pub dedupe: bool,
    /// Player turns a fight may take before the run ends as [`RunEnd::FightCapped`]; 0 for no
    /// cap.
    pub turn_cap: u16,
    /// Agent decisions a run may take before it ends as [`RunEnd::DecisionCapped`]; 0 for no
    /// cap.
    pub decision_cap: u32,
    /// Write [`RunEnvs::events`].
    pub events: bool,
    /// Count saturated observations and unmapped decisions ([`RunStepStats`]).
    pub counters: bool,
    /// Write [`RunEnvs::priv_obs`] (what the player cannot see, for the value head only).
    pub priv_obs: bool,
    /// With `dedupe`: a combat card choice's key includes the pile of the card (RLN14), so a
    /// hand card and a draw-pile card never merge. Off by default: it changes the classes, so the
    /// action space, of a run in progress.
    pub pile_key: bool,
    /// Write the run observation's v2 section ([`crate::obs_v2`], RLN16): intent links, the
    /// honest forecast and the hand's damage and block previews.
    pub obs_v2: bool,
    /// With `obs_v2`, write its plan column ([`crate::plan_column::write_plan`], RLN22). The
    /// FFI sets it with `obs_v2`; off, the section's plan bytes stay zero (a cost baseline).
    pub plan_column: bool,
    /// With `obs_v2`, write the full-view section ([`crate::obs_full`], RLN30): each
    /// creature's past moves.
    pub obs_full: bool,
    /// Offer discarding a potion where a human makes room for one ([`crate::run_action::discard_offers`],
    /// RLN32): a rewards screen with a potion or a shop with an affordable one, belt full. Off, the
    /// masks are as before.
    pub discard_potion: bool,
    /// With `obs_v2` and the plan column: plays per plan, 1 ([`crate::plan_column::OBS_DEPTH`])
    /// or 2; 0 reads as 1 (RLN32).
    pub plan_depth: u8,
    /// Write the event section ([`crate::obs_events`], RLN33): each event and ancient
    /// option's effects as numbers.
    pub obs_events: bool,
}

impl RunOptions {
    /// All off (the default).
    pub const OFF: Self = Self {
        dedupe: false,
        turn_cap: 0,
        decision_cap: 0,
        events: false,
        counters: false,
        priv_obs: false,
        pile_key: false,
        obs_v2: false,
        plan_column: false,
        obs_full: false,
        discard_potion: false,
        plan_depth: 0,
        obs_events: false,
    };

    /// The plan's settings: classes, a 100-turn fight cap, a 6,000-decision run cap, events and
    /// counters.
    pub const E2E: Self = Self {
        dedupe: true,
        turn_cap: 100,
        decision_cap: 6_000,
        events: true,
        counters: true,
        priv_obs: false,
        pile_key: false,
        obs_v2: false,
        plan_column: false,
        obs_full: false,
        discard_potion: false,
        plan_depth: 0,
        obs_events: false,
    };
}

/// What happened to an env's episode in one step, for reward shaping ([`RunEnvs::events`]).
///
/// A step is the agent's action and the automatic play after it. Events while an env resets to
/// a new episode (before its first decision) are not reported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct RunEvents {
    /// Floors entered.
    pub floors: u8,
    /// Bosses killed (an act with two bosses counts each).
    pub bosses: u8,
    /// Bit `a` set when a boss of act index `a` was killed.
    pub boss_acts: u8,
    /// 1 when the run was won.
    pub won: u8,
}

/// Saturation kinds of [`RunStepStats::saturated`].
///
/// Any of the others, then the combat cards (over [`crate::obs::CARDS`]), the deck (over the run observation's [`DECK`], or in a fight
/// the static part's [`crate::obs::DECK`]), creatures ([`crate::obs::CREATURES`]), a creature's
/// powers ([`crate::obs::POWERS`]), a combat card choice's options ([`OPTION_SLOTS`]), relics
/// ([`RELICS`]) and map points ([`MAP_POINTS`]).
pub const SATURATIONS: [&str; 8] = [
    "any",
    "cards",
    "deck",
    "creatures",
    "powers",
    "choice",
    "relics",
    "map",
];

/// With `obs_v2` and the plan column, in a fight: writes the plan section of `obs` from the mask
/// and the legal list (`scratch` holds the combat actions the planner scores).
pub(crate) fn write_plan_section(
    run: &Run,
    options: RunOptions,
    mask: &RunMask,
    legal: &[RunAction],
    scratch: &mut Vec<(usize, Action)>,
    obs: &mut [u8],
) {
    if !(options.obs_v2 && options.plan_column) {
        return;
    }
    let RunDecision::Combat(combat) = run.pending_decision() else {
        return;
    };
    scratch.clear();
    scratch.extend((0..ACTIONS).filter_map(|index| {
        let position = mask.position(index)?;
        match &legal[position] {
            RunAction::Combat(a) => Some((index, a.clone())),
            _ => None,
        }
    }));
    write_plan_at(
        combat,
        big_fight(run),
        scratch,
        usize::from(options.plan_depth).max(OBS_DEPTH),
        &mut obs[V2_AT + PLAN_AT..][..ACTIONS * PLAN_BYTES],
    );
}

/// What the envs play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunConfig {
    /// The character, or `None` for seed `s` playing `Character::ALL[s % 5]`.
    pub character: Option<Character>,
    /// 0 to 10.
    pub ascension: u8,
    pub combat: CombatMode,
    /// Whether the combat section's intents are written.
    pub intents: bool,
    /// The end-to-end options (all off by default).
    pub options: RunOptions,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            character: Some(Character::Necrobinder),
            ascension: 0,
            combat: CombatMode::Agent,
            intents: true,
            options: RunOptions::OFF,
        }
    }
}

impl RunConfig {
    /// The character of seed `seed`.
    #[must_use]
    pub fn character_of(&self, seed: u64) -> Character {
        self.character
            .unwrap_or_else(|| Character::ALL[usize::try_from(seed % 5).unwrap_or_default()])
    }

    /// A new run on `seed`.
    ///
    /// # Errors
    /// As `Run::new`.
    pub fn run(&self, seed: u64) -> Result<Run, sts2_core::run::RunError> {
        Run::new(
            self.character_of(seed),
            &format!("RUN{seed}"),
            self.ascension,
            UnlockState::all(),
            Act1::Random,
        )
    }
}

/// The episode serial of a practice episode (it takes none of the source's).
pub const PRACTICE_EPISODE: u64 = u64::MAX;

/// A finished episode, as [`RunEnvs::step_with`] reports it before the env resets.
#[derive(Clone, Copy, Debug)]
pub struct RunEpisodeEnd<'a> {
    pub env: usize,
    /// The episode's serial (the `k` of [`SeedSource::seed`]) and seed; for a practice
    /// episode [`PRACTICE_EPISODE`] and its reseed.
    pub episode: u64,
    pub seed: u64,
    pub end: RunEnd,
    /// The total floor reached.
    pub floor: u16,
    /// Engine actions; of those, the agent's, the automatic ones and the resolver's.
    pub actions: u64,
    pub decisions: u64,
    pub auto: u64,
    pub resolved: u64,
    /// Bosses killed (counted with [`RunOptions::events`] only, else 0).
    pub bosses: u32,
    /// The final run (`None` if `Run::new` failed).
    pub run: Option<&'a Run>,
    /// The cell of a practice episode.
    pub cell: Option<Cell>,
    /// Every engine action of the episode in order (the agent's, the automatic ones and the
    /// resolver's); empty unless [`RunEnvs::record_actions`] is on.
    pub log: &'a [RunAction],
}

/// What one [`RunEnvs::step`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunStepStats {
    /// Engine actions applied: the agent's, the automatic ones and the resolver's.
    pub actions: u64,
    /// The agent's actions applied.
    pub decisions: u64,
    /// Single-option decisions taken automatically.
    pub auto: u64,
    /// Actions the combat resolver took.
    pub resolved: u64,
    /// Listed actions left out of the masks because the engine would refuse them, and resolver
    /// positions where it refused every action.
    pub refused: u64,
    /// Legal actions the fixed space has no index for, over the masks written.
    pub unmapped: u64,
    /// Episodes that ended; of those, won, capped (by any cap) and stuck.
    pub ended: u32,
    pub won: u32,
    pub capped: u32,
    pub stuck: u32,
    /// Envs reset to a new episode.
    pub reset: u32,
    /// Of the capped episodes, those ended by [`RunOptions::turn_cap`] and
    /// [`RunOptions::decision_cap`].
    pub fight_capped: u32,
    pub decision_capped: u32,
    /// With [`RunOptions::dedupe`]: indices merged into an earlier member's class, classes no
    /// member of which has an index, and decisions taken automatically because their options
    /// made one class.
    pub merged: u64,
    pub unmapped_classes: u64,
    pub auto_class: u64,
    /// With [`RunOptions::counters`]: agent decisions whose legal list had an action without an
    /// index, and agent decisions whose observation saturated each kind of [`SATURATIONS`].
    pub unmapped_decisions: u64,
    pub saturated: [u64; 8],
    /// With [`RunOptions::dedupe`]: merged members that the observation shows differently from
    /// their class's first member, per field of [`crate::obs::RECORD_FIELDS`]. Any is a merge-key
    /// bug (the agent would see one card and play another); debug builds panic on it.
    pub member_mismatch: [u64; RECORD_FIELDS.len()],
}

/// Why [`RunEnvs::step`] refused a batch; nothing was applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStepError {
    /// The batch does not have one action per env.
    Length { expected: usize, got: usize },
    /// The action of a live env is not set in its mask.
    Illegal { env: usize, action: u16 },
}

impl std::fmt::Display for RunStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Length { expected, got } => write!(f, "expected {expected} actions, got {got}"),
            Self::Illegal { env, action } => write!(f, "action {action} is not legal in env {env}"),
        }
    }
}

impl std::error::Error for RunStepError {}

/// What the envs of one pass did, summed.
#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    actions: u64,
    decisions: u64,
    auto: u64,
    resolved: u64,
    refused: u64,
    unmapped: u64,
    merged: u64,
    unmapped_classes: u64,
    auto_class: u64,
    unmapped_decisions: u64,
    saturated: [u64; 8],
    member_mismatch: [u64; RECORD_FIELDS.len()],
}

impl std::ops::Add for Counts {
    type Output = Self;

    fn add(self, o: Self) -> Self {
        let mut saturated = self.saturated;
        for (s, o) in saturated.iter_mut().zip(o.saturated) {
            *s += o;
        }
        let mut member_mismatch = self.member_mismatch;
        for (s, o) in member_mismatch.iter_mut().zip(o.member_mismatch) {
            *s += o;
        }
        Self {
            actions: self.actions + o.actions,
            decisions: self.decisions + o.decisions,
            auto: self.auto + o.auto,
            resolved: self.resolved + o.resolved,
            refused: self.refused + o.refused,
            unmapped: self.unmapped + o.unmapped,
            merged: self.merged + o.merged,
            unmapped_classes: self.unmapped_classes + o.unmapped_classes,
            auto_class: self.auto_class + o.auto_class,
            unmapped_decisions: self.unmapped_decisions + o.unmapped_decisions,
            saturated,
            member_mismatch,
        }
    }
}

/// One env's slices of the batch buffers.
struct Slots<'a> {
    obs: &'a mut [u8],
    privs: &'a mut [u8],
    mask: &'a mut [u64],
    counts: &'a mut [u8],
}

/// The saturation kinds of `run`'s position as a bitmask over [`SATURATIONS`] (bit 0 when any
/// other is set).
fn saturation(run: &Run) -> u8 {
    use crate::obs;
    let s = run.state();
    let mut bits = u8::from(s.deck.len() > DECK) << 2
        | u8::from(s.relics.len() > RELICS) << 6
        | u8::from(
            s.map.points().len() + 2 + usize::from(s.map.second_boss().is_some()) > MAP_POINTS,
        ) << 7;
    if let Some(combat) = run.combat() {
        let c = combat.state();
        bits |= u8::from(c.cards.len() > obs::CARDS) << 1
            | u8::from(c.deck.len() > obs::DECK) << 2
            | u8::from(c.allies.len() + c.enemies.len() > obs::CREATURES) << 3
            | u8::from(
                c.allies
                    .iter()
                    .chain(&c.enemies)
                    .any(|&id| c.creature(id).powers.len() > obs::POWERS),
            ) << 4
            | u8::from(matches!(
                combat.pending_decision(),
                Some(DecisionRequest::ChooseCards { options, .. }) if options.len() > OPTION_SLOTS
            )) << 5
            | u8::from(c.player.relics.len() > obs::RELICS) << 6;
    }
    bits | u8::from(bits != 0)
}

/// Merges `mask` (filled from `legal`, `run`'s list) into action classes; `counts` gets the
/// kept indices' member counts.
pub(crate) fn dedupe(
    run: &Run,
    legal: &[RunAction],
    mask: &mut RunMask,
    counts: &mut [u8],
    scratch: &mut DedupeScratch,
    pile_key: bool,
) -> Dedupe {
    let s = run.state();
    let decision = run.pending_decision();
    let combat = run.combat();
    mask.dedupe_checked(
        legal,
        |a| match (combat, a) {
            (Some(c), RunAction::Combat(a)) => action::index_of(c, a),
            (_, a) => run_index_of(run, a),
        },
        |a| run_key_with(s, &decision, a, pile_key),
        |a, b| a.same(b, combat),
        |a| run.accepts(a),
        |a, b| match (combat, a, b) {
            (Some(c), RunAction::Combat(a), RunAction::Combat(b)) => record_mismatch(c, a, b),
            _ => 0,
        },
        counts,
        scratch,
    )
}

/// The act index of `run`'s boss fight, if it is in one.
fn boss_fight(run: &Run) -> Option<usize> {
    let s = run.state();
    (run.combat().is_some()
        && matches!(
            s.room,
            Room::Combat {
                room_type: RoomType::Boss,
                ..
            }
        ))
    .then_some(s.act_index)
}

/// One environment.
#[derive(Debug)]
struct RunEnv {
    /// The run of the episode (`None` before the first, or if `Run::new` failed).
    run: Option<Run>,
    observer: RunObserver,
    privileged: PrivObserver,
    resolver: Option<Box<dyn Resolver>>,
    legal: Vec<RunAction>,
    mask: RunMask,
    /// With `obs_v2`, the masked combat actions the plan column scores (scratch).
    plan_actions: Vec<(usize, Action)>,
    /// The episode's serial and seed; `None` once the source has no more.
    episode: Option<(u64, u64)>,
    actions: u64,
    decisions: u64,
    auto: u64,
    resolved: u64,
    /// Bosses killed this episode (with [`RunOptions::events`]).
    bosses: u32,
    end: RunEnd,
    /// A new episode was assigned in this step.
    fresh: bool,
    options: RunOptions,
    /// This step's events (with [`RunOptions::events`]).
    events: RunEvents,
    scratch: DedupeScratch,
    /// Whether a normal run captures practice snapshots.
    capture: bool,
    /// Snapshots taken since the batch last collected them.
    captured: Vec<Snapshot>,
    /// The practice segment this episode plays, if any.
    segment: Option<(Cell, SegmentEnd)>,
    /// The practice start the next reset plays.
    start: Option<PracticeStart>,
    /// The gym start the next reset builds (RLN43).
    gym: Option<GymTicket>,
    /// The gym episode's cell code (7 + [`crate::human::GymCell::index`]), if it plays one.
    gym_code: Option<u8>,
    /// Every action applied this episode, when [`RunEnvs::record_actions`] is on.
    log: Option<Vec<RunAction>>,
}

impl RunEnv {
    fn new(config: RunConfig) -> Self {
        Self {
            run: None,
            observer: RunObserver::new(config.intents)
                .with_v2(config.options.obs_v2)
                .with_full(config.options.obs_full)
                .with_events(config.options.obs_events),
            privileged: PrivObserver::default(),
            resolver: config.combat.resolver(),
            legal: Vec::new(),
            mask: RunMask::new(),
            plan_actions: Vec::new(),
            episode: None,
            actions: 0,
            decisions: 0,
            auto: 0,
            resolved: 0,
            bosses: 0,
            end: RunEnd::None,
            fresh: false,
            options: config.options,
            events: RunEvents::default(),
            scratch: DedupeScratch::default(),
            capture: false,
            captured: Vec::new(),
            segment: None,
            start: None,
            gym: None,
            gym_code: None,
            log: None,
        }
    }

    /// Whether a practice segment stops at this position: its first outside combat.
    fn practice_stop(&self, run: &Run) -> bool {
        (self.gym_code.is_some() || matches!(self.segment, Some((_, SegmentEnd::FirstOutOfCombat))))
            && run.combat().is_none()
    }

    /// Records a boss kill if `boss` (the act of the boss fight before an action, if any) ended
    /// in anything but a death or unported content.
    fn track(&mut self, boss: Option<usize>) {
        let Some(act) = boss else {
            return;
        };
        let run = self.run.as_ref().expect("a live run");
        if run.combat().is_none() && !matches!(run.pending_decision(), RunDecision::GameOver) {
            self.bosses += 1;
            self.events.bosses = self.events.bosses.saturating_add(1);
            self.events.boss_acts |= u32::try_from(act)
                .ok()
                .and_then(|a| 1_u8.checked_shl(a))
                .unwrap_or(0);
        }
    }

    /// Whether the env takes an action this step.
    const fn acts(&self) -> bool {
        self.episode.is_some() && matches!(self.end, RunEnd::None) && self.run.is_some()
    }

    /// Applies the legal action at list position `position`, one the mask holds.
    fn apply(&mut self, position: usize, counts: &mut Counts) {
        let run = self.run.as_mut().expect("a live run");
        let boss = if self.options.events {
            boss_fight(run)
        } else {
            None
        };
        if self.capture
            && self.segment.is_none()
            && self.gym_code.is_none()
            && let Some(snap) = practice::capture(run, &self.legal[position])
        {
            self.captured.push(snap);
        }
        if let Some(log) = &mut self.log {
            log.push(self.legal[position].clone());
        }
        run.apply(self.legal[position].clone())
            .expect("the mask holds only actions the engine accepts");
        self.actions += 1;
        counts.actions += 1;
        self.track(boss);
    }

    /// How the run ended, if it has.
    fn terminal(&self) -> Option<RunEnd> {
        let run = self.run.as_ref()?;
        let options = &self.options;
        match run.pending_decision() {
            RunDecision::GameOver => Some(RunEnd::Died),
            RunDecision::Victory => Some(RunEnd::Victory),
            RunDecision::Stopped(_) => Some(RunEnd::Unsupported),
            _ if self.actions >= MAX_RUN_ACTIONS => Some(RunEnd::Capped),
            _ if options.decision_cap > 0 && self.decisions >= u64::from(options.decision_cap) => {
                Some(RunEnd::DecisionCapped)
            }
            RunDecision::Combat(combat)
                if options.turn_cap > 0
                    && combat.state().player.turn_number > u32::from(options.turn_cap) =>
            {
                Some(RunEnd::FightCapped)
            }
            _ => None,
        }
    }

    /// The resolver's next action in the fight (it may answer a card choice with an action the
    /// list does not hold), or the first legal one the engine accepts.
    fn resolve(&mut self, counts: &mut Counts) -> bool {
        let run = self.run.as_mut().expect("a live run");
        let boss = if self.options.events {
            boss_fight(run)
        } else {
            None
        };
        let resolver = self.resolver.as_mut().expect("a resolver");
        let action = resolver.act(run, &self.legal);
        let log = &mut self.log;
        let applied = if run.apply(action.clone()).is_ok() {
            if let Some(log) = log {
                log.push(action);
            }
            true
        } else {
            self.legal.iter().find(|a| run.accepts(a)).is_some_and(|a| {
                run.apply(a.clone()).expect("an accepted action");
                if let Some(log) = log {
                    log.push(a.clone());
                }
                true
            })
        };
        if applied {
            self.actions += 1;
            self.resolved += 1;
            counts.actions += 1;
            counts.resolved += 1;
            self.track(boss);
        } else {
            counts.refused += 1;
        }
        applied
    }

    /// Plays on from the current position (resolving fights, taking single options) until a
    /// decision with more than one option or the episode's end; then writes the mask and, if
    /// the episode goes on, the observation.
    fn settle(&mut self, slots: &mut Slots<'_>, counts: &mut Counts) {
        let mut unmapped;
        loop {
            if let Some(end) = self.terminal() {
                self.end = end;
                return;
            }
            let run = self.run.as_ref().expect("a live run");
            run.legal_actions_into(&mut self.legal);
            if self.options.discard_potion {
                discard_offers(run, &mut self.legal);
            }
            if self.resolver.is_some() && matches!(run.pending_decision(), RunDecision::Combat(_)) {
                if self.legal.is_empty() || !self.resolve(counts) {
                    self.end = RunEnd::Stuck;
                    return;
                }
                continue;
            }
            unmapped = self.mask.fill_run(run, &self.legal);
            counts.unmapped += u64::from(unmapped);
            counts.refused += u64::from(self.mask.retain_accepted(run, &self.legal));
            debug_assert_eq!(
                check_run_mask(run, &self.legal, &self.mask),
                Ok(()),
                "the run mask disagrees with the legal list"
            );
            if self.options.dedupe {
                let options = self.mask.count();
                let d = dedupe(
                    run,
                    &self.legal,
                    &mut self.mask,
                    slots.counts,
                    &mut self.scratch,
                    self.options.pile_key,
                );
                debug_assert_eq!(
                    d.mismatches,
                    [0; RECORD_FIELDS.len()],
                    "merged members look different to the agent (fields {RECORD_FIELDS:?})"
                );
                for (n, m) in counts.member_mismatch.iter_mut().zip(d.mismatches) {
                    *n += u64::from(m);
                }
                counts.merged += u64::from(d.merged);
                counts.unmapped_classes += u64::from(d.unmapped_classes);
                if options > 1 && self.mask.count() == 1 {
                    counts.auto_class += 1;
                }
            }
            let stop = self.practice_stop(run);
            match self.mask.count() {
                0 => {
                    self.end = RunEnd::Stuck;
                    return;
                }
                1 if !stop => {
                    let index = self.mask.nth(0).expect("one bit");
                    let position = self.mask.position(index).expect("set");
                    self.apply(position, counts);
                    self.auto += 1;
                    counts.auto += 1;
                }
                _ => break,
            }
        }
        slots.mask.copy_from_slice(self.mask.bits());
        let run = self.run.as_ref().expect("a live run");
        self.observer.observe(run, slots.obs);
        write_plan_section(
            run,
            self.options,
            &self.mask,
            &self.legal,
            &mut self.plan_actions,
            slots.obs,
        );
        if self.options.priv_obs {
            self.privileged.write(run, slots.privs);
        }
        if self.options.counters {
            counts.unmapped_decisions += u64::from(unmapped > 0);
            let bits = saturation(run);
            for (k, count) in counts.saturated.iter_mut().enumerate() {
                *count += u64::from(bits >> k & 1);
            }
        }
        if self.practice_stop(run) {
            self.end = if self.gym_code.is_some() {
                RunEnd::GymEnd
            } else {
                RunEnd::PracticeEnd
            };
        }
    }

    /// Applies the agent's `action` (checked against the mask), then settles.
    fn step(&mut self, action: u16, slots: &mut Slots<'_>) -> Counts {
        let mut counts = Counts::default();
        self.events = RunEvents::default();
        if action == HOLD || !self.acts() {
            return counts;
        }
        let floor = if self.options.events { self.floor() } else { 0 };
        let index = usize::from(action);
        let position = self
            .mask
            .position(index)
            .expect("the batch was checked against the masks");
        // CB1: an engine panic inside one episode (e.g. a `Decimal` overflow from a scaling
        // combo the agent found) ends that episode as unsupported instead of the whole batch.
        let played = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.apply(position, &mut counts);
            self.decisions += 1;
            counts.decisions = 1;
            self.settle(slots, &mut counts);
        }));
        if played.is_err() {
            self.end = RunEnd::Unsupported;
        }
        if self.options.events {
            self.events.floors =
                u8::try_from(self.floor().saturating_sub(floor)).unwrap_or(u8::MAX);
            self.events.won = u8::from(self.end == RunEnd::Victory);
        }
        counts
    }

    /// Starts the fresh episode: a new run on its seed, settled to its first decision.
    fn reset(&mut self, config: RunConfig, slots: &mut Slots<'_>) -> Counts {
        self.fresh = false;
        self.end = RunEnd::None;
        (self.actions, self.decisions, self.auto, self.resolved) = (0, 0, 0, 0);
        if let Some(log) = &mut self.log {
            log.clear();
        }
        self.bosses = 0;
        self.observer.reset();
        self.privileged.reset();
        slots.mask.fill(0);
        slots.obs.fill(0);
        slots.privs.fill(0);
        if self.options.dedupe {
            slots.counts.fill(0);
        }
        let Some((_, seed)) = self.episode else {
            self.run = None;
            return Counts::default();
        };
        if let Some(resolver) = &mut self.resolver {
            resolver.reset(seed);
        }
        let mut counts = Counts::default();
        let start = self.start.take();
        let gym = self.gym.take();
        self.segment = start.as_ref().map(|s| (s.cell, s.end));
        self.gym_code = gym.as_ref().map(GymTicket::code);
        self.run = match (start, gym) {
            (Some(s), _) => Some(s.run),
            // A start that panics in its set-up (content the check missed) ends as
            // unsupported instead of poisoning the batch.
            (None, Some(t)) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t.build()))
                .ok()
                .and_then(Result::ok),
            (None, None) => config.run(seed).ok(),
        };
        if self.run.is_some() {
            self.settle(slots, &mut counts);
        } else {
            self.end = RunEnd::Unsupported;
        }
        // Events before the first decision belong to no step.
        self.events = RunEvents::default();
        counts
    }

    /// Becomes a copy of `from`'s position ([`RunEnvs::copy_env`]): the run, the episode and
    /// its counters, and the observers' state (CB2: the full view's past-move intents are the
    /// observer's history, so a copy that kept its own observer showed another fight's), then,
    /// with `reseed`, the fight determinized. [`RunEnv::settle_copy`] writes its slots.
    fn copy_from(&mut self, from: &Self, reseed: Option<u64>) {
        self.run.clone_from(&from.run);
        if let (Some(seed), Some(run)) = (reseed, self.run.as_mut()) {
            run.determinize_combat(seed);
        }
        self.observer.clone_from(&from.observer);
        self.privileged.clone_from(&from.privileged);
        (self.episode, self.actions, self.decisions) = (from.episode, from.actions, from.decisions);
        (self.auto, self.resolved, self.bosses) = (from.auto, from.resolved, from.bosses);
        (self.end, self.segment, self.gym_code) = (from.end, from.segment, from.gym_code);
        self.log.clone_from(&from.log);
        (self.start, self.fresh, self.events, self.gym) = (None, false, RunEvents::default(), None);
    }

    /// After [`RunEnv::copy_from`]: settles a live copy, writing its observation and mask.
    fn settle_copy(&mut self, slots: &mut Slots<'_>) {
        if self.run.is_some() && self.end == RunEnd::None {
            self.settle(slots, &mut Counts::default());
        }
    }

    /// The total floor reached.
    fn floor(&self) -> u16 {
        self.run.as_ref().map_or(0, |run| {
            u16::try_from(run.state().total_floor()).unwrap_or(u16::MAX)
        })
    }
}

/// The batch buffers each env gets a slice of.
struct Buffers<'a> {
    obs: &'a mut [u8],
    privs: &'a mut [u8],
    masks: &'a mut [u64],
    counts: &'a mut [u8],
}

/// A block of envs with their slices of the batch buffers.
struct Block<'a> {
    envs: &'a mut [RunEnv],
    buffers: Buffers<'a>,
}

/// The envs a worker of [`each_env`] takes at a time: small, so that a thread that drew
/// cheap envs (map decisions) takes more while another is still in a boss fight or a reset.
const BLOCK: usize = 8;

/// Runs `f` on every env with its index and slots, on up to `threads` scoped threads that take
/// blocks of [`BLOCK`] envs from a shared cursor until none is left; the sum of its results.
/// Which thread runs an env changes nothing: each env owns its state and its slots, and the
/// sum is commutative.
fn each_env<F>(threads: usize, envs: &mut [RunEnv], buffers: Buffers<'_>, f: F) -> Counts
where
    F: Fn(usize, &mut RunEnv, &mut Slots<'_>) -> Counts + Sync,
{
    let run = |first: usize, envs: &mut [RunEnv], b: Buffers<'_>| {
        envs.iter_mut()
            .zip(b.obs.as_chunks_mut::<RUN_OBS_BYTES>().0)
            .zip(b.privs.as_chunks_mut::<PRIV_BYTES>().0)
            .zip(b.masks.as_chunks_mut::<RUN_MASK_WORDS>().0)
            .zip(b.counts.as_chunks_mut::<RUN_ACTIONS>().0)
            .enumerate()
            .map(|(i, ((((env, obs), privs), mask), counts))| {
                f(
                    first + i,
                    env,
                    &mut Slots {
                        obs,
                        privs,
                        mask,
                        counts,
                    },
                )
            })
            .fold(Counts::default(), |sum, c| sum + c)
    };
    let timing = step_timing::enabled().then(Instant::now);
    if threads <= 1 || envs.len() < 2 {
        let counts = run(0, envs, buffers);
        if let Some(start) = timing {
            step_timing::record(start, &[start.elapsed()]);
        }
        return counts;
    }
    // Each block behind its own (uncontended) lock, taken once by whichever thread draws it.
    let blocks: Vec<Mutex<Option<Block<'_>>>> = envs
        .chunks_mut(BLOCK)
        .zip(buffers.obs.chunks_mut(BLOCK * RUN_OBS_BYTES))
        .zip(buffers.privs.chunks_mut(BLOCK * PRIV_BYTES))
        .zip(buffers.masks.chunks_mut(BLOCK * RUN_MASK_WORDS))
        .zip(buffers.counts.chunks_mut(BLOCK * RUN_ACTIONS))
        .map(|((((envs, obs), privs), masks), counts)| {
            Mutex::new(Some(Block {
                envs,
                buffers: Buffers {
                    obs,
                    privs,
                    masks,
                    counts,
                },
            }))
        })
        .collect();
    let cursor = AtomicUsize::new(0);
    let worker = || {
        let start = timing.map(|_| Instant::now());
        let mut sum = Counts::default();
        loop {
            let b = cursor.fetch_add(1, Ordering::Relaxed);
            let Some(block) = blocks.get(b) else {
                break;
            };
            let taken = block.lock().map_or(None, |mut slot| slot.take());
            if let Some(Block { envs, buffers }) = taken {
                sum = sum + run(b * BLOCK, envs, buffers);
            }
        }
        (sum, start.map_or(Duration::ZERO, |s| s.elapsed()))
    };
    let (counts, busy): (Counts, Vec<Duration>) = thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every thread starts before the first join"
        )]
        let handles: Vec<_> = (0..threads.min(blocks.len()))
            .map(|_| scope.spawn(worker))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("an env thread panicked"))
            .fold(
                (Counts::default(), Vec::new()),
                |(sum, mut busy), (c, d)| {
                    busy.push(d);
                    (sum + c, busy)
                },
            )
    });
    if let Some(start) = timing {
        step_timing::record(start, &busy);
    }
    counts
}

/// A batch of run environments.
#[derive(Debug)]
pub struct RunEnvs {
    batch: Vec<RunEnv>,
    seeds: SeedSource,
    config: RunConfig,
    /// The next episode's serial.
    next: u64,
    threads: usize,
    obs: Vec<u8>,
    /// With [`RunOptions::priv_obs`], the privileged buffers (else zeros).
    privs: Vec<u8>,
    /// The observation of the episode that ended in the last step, before its reset.
    final_obs: Vec<u8>,
    /// Its mask and (with [`RunOptions::priv_obs`]) privileged buffer.
    final_masks: Vec<u64>,
    final_privs: Vec<u8>,
    masks: Vec<u64>,
    /// With [`RunOptions::dedupe`], per env and index, the kept index's class size.
    class_counts: Vec<u8>,
    done: Vec<u8>,
    won: Vec<u8>,
    ends: Vec<u8>,
    floors: Vec<u16>,
    events: Vec<RunEvents>,
    /// Practice ([`RunEnvs::set_practice`]); `None` until set.
    practice: Option<Pool>,
    /// The combat gym (RLN43, [`RunEnvs::set_gym`]); `None` until set.
    gym: Option<Gym>,
    /// Per env, the cell code (0 a normal run, else [`Cell::index`] + 1) of its episode, and of
    /// the episode that ended in the last step.
    cells: Vec<u8>,
    ended_cells: Vec<u8>,
}

/// The cell code of [`RunEnvs::cells`].
fn cell_code(env: &RunEnv) -> u8 {
    env.gym_code.unwrap_or_else(|| {
        env.segment.map_or(0, |(cell, _)| {
            u8::try_from(cell.index() + 1).expect("fewer than 255 cells")
        })
    })
}

impl RunEnvs {
    /// `n` envs playing the runs of `seeds` (in order) as `config` says, reset to their first
    /// episodes.
    #[must_use]
    pub fn new(n: usize, seeds: SeedSource, config: RunConfig) -> Self {
        let mut envs = Self {
            batch: (0..n).map(|_| RunEnv::new(config)).collect(),
            seeds,
            config,
            next: 0,
            threads: 1,
            obs: vec![0; n * RUN_OBS_BYTES],
            privs: vec![0; n * PRIV_BYTES],
            final_obs: vec![0; n * RUN_OBS_BYTES],
            final_masks: vec![0; n * RUN_MASK_WORDS],
            final_privs: vec![0; n * PRIV_BYTES],
            masks: vec![0; n * RUN_MASK_WORDS],
            class_counts: vec![0; n * RUN_ACTIONS],
            done: vec![0; n],
            won: vec![0; n],
            ends: vec![0; n],
            floors: vec![0; n],
            events: vec![RunEvents::default(); n],
            practice: None,
            gym: None,
            cells: vec![0; n],
            ended_cells: vec![0; n],
        };
        envs.reset_all();
        envs
    }

    /// Records (or stops recording) every engine action of each episode, reported in
    /// [`RunEpisodeEnd::log`]. Off by default; a log keeps the episode's actions until its reset.
    /// Takes effect from the next episode.
    pub fn record_actions(&mut self, on: bool) {
        for env in &mut self.batch {
            env.log = on.then(Vec::new);
        }
    }

    /// Turns practice on with `config`, or changes its settings (the pool, its generator and
    /// counts stay); `None` stops capturing and starting (the pool stays, unused). It takes
    /// effect at the next capture and the next episode end.
    pub fn set_practice(&mut self, config: Option<PracticeConfig>) {
        match (config, &mut self.practice) {
            (Some(c), Some(pool)) => pool.configure(c),
            (Some(c), None) => self.practice = Some(Pool::new(c)),
            (None, Some(pool)) => pool.configure(PracticeConfig {
                slots: 0,
                capture: false,
                ..*pool.config()
            }),
            (None, None) => {}
        }
        let capture = self.practice.as_ref().is_some_and(|p| p.config().capture);
        for env in &mut self.batch {
            env.capture = capture;
        }
    }

    /// Loads the gym's starts (RLN43, [`crate::human`]), replacing any loaded before, with no
    /// slots until [`RunEnvs::set_gym`]; returns how many.
    pub fn load_gym(&mut self, starts: Vec<HumanStart>) -> usize {
        let config = self
            .gym
            .as_ref()
            .map_or_else(GymConfig::default, |g| GymConfig {
                slots: 0,
                ..*g.config()
            });
        let gym = Gym::new(starts, config);
        let n = gym.len();
        self.gym = Some(gym);
        n
    }

    /// New gym settings (after [`RunEnvs::load_gym`]; ignored before); `None` gives it no slots.
    pub fn set_gym(&mut self, config: Option<GymConfig>) {
        if let Some(gym) = &mut self.gym {
            gym.configure(config.unwrap_or_else(|| GymConfig {
                slots: 0,
                ..*gym.config()
            }));
        }
    }

    /// The gym, if loaded.
    #[must_use]
    pub const fn gym(&self) -> Option<&Gym> {
        self.gym.as_ref()
    }

    /// The practice pool, if practice was ever set.
    #[must_use]
    pub const fn practice(&self) -> Option<&Pool> {
        self.practice.as_ref()
    }

    /// The practice pool, for filing snapshots made elsewhere ([`Pool::add`]; tests and tools).
    pub const fn practice_mut(&mut self) -> Option<&mut Pool> {
        self.practice.as_mut()
    }

    /// Files the snapshots the envs took (in env order) in the pool.
    fn collect_snapshots(&mut self) {
        for env in &mut self.batch {
            for snap in env.captured.drain(..) {
                if let Some(pool) = &mut self.practice {
                    pool.add(snap);
                }
            }
        }
    }

    /// Writes [`RunEnvs::cells`].
    fn write_cells(&mut self) {
        for (code, env) in self.cells.iter_mut().zip(&self.batch) {
            *code = cell_code(env);
        }
    }

    /// Replaces env `i`'s combat resolver (a hybrid mode's own policy); it takes effect at the
    /// next fight. `None` lets the agent play combat.
    pub fn set_resolver(&mut self, i: usize, resolver: Option<Box<dyn Resolver>>) {
        self.batch[i].resolver = resolver;
    }

    /// The number of envs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.batch.len()
    }

    /// Whether there are no envs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Steps the envs on up to `t` threads (at least 1).
    pub fn threads(&mut self, t: usize) {
        self.threads = t.max(1);
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &RunConfig {
        &self.config
    }

    /// Hands env `i` a practice start, if it is a practice slot and a cell is active, else the
    /// next episode of the source.
    fn assign(&mut self, i: usize) {
        if let Some(pool) = &mut self.practice
            && i < pool.config().slots
            && let Some(start) = pool.start()
        {
            let env = &mut self.batch[i];
            env.episode = Some((PRACTICE_EPISODE, start.reseed));
            env.start = Some(start);
            env.fresh = true;
            return;
        }
        if let Some(gym) = &mut self.gym
            && gym.covers(i)
            && let Some(ticket) = gym.draw()
        {
            let env = &mut self.batch[i];
            env.episode = Some((PRACTICE_EPISODE, ticket.seed));
            env.gym = Some(ticket);
            env.fresh = true;
            return;
        }
        let k = self.next;
        self.next += 1;
        let env = &mut self.batch[i];
        env.episode = self.seeds.seed(k).map(|seed| (k, seed));
        env.fresh = true;
    }

    /// Resets the envs marked fresh.
    fn reset_fresh(&mut self) -> Counts {
        let config = self.config;
        each_env(
            self.threads,
            &mut self.batch,
            Buffers {
                obs: &mut self.obs,
                privs: &mut self.privs,
                masks: &mut self.masks,
                counts: &mut self.class_counts,
            },
            |_, env, slots| {
                if env.fresh {
                    env.reset(config, slots)
                } else {
                    Counts::default()
                }
            },
        )
    }

    /// Starts a new episode in every env (the next seeds of the source, in env order).
    pub fn reset_all(&mut self) {
        for i in 0..self.batch.len() {
            self.assign(i);
        }
        self.clear_ended();
        self.reset_fresh();
        self.write_cells();
    }

    /// Zeroes the buffers about the episodes that ended in the last step.
    fn clear_ended(&mut self) {
        for buffer in [
            &mut self.done,
            &mut self.won,
            &mut self.ends,
            &mut self.ended_cells,
        ] {
            buffer.fill(0);
        }
        self.floors.fill(0);
        self.events.fill(RunEvents::default());
        self.final_obs.fill(0);
        self.final_masks.fill(0);
        self.final_privs.fill(0);
    }

    /// Starts env `i` on a fresh run of `seeds[i]` (episode serial `i`), and hands the
    /// source's episodes from serial `next` on to the runs that end after this: a learner's
    /// resume ([`HOLD`]).
    ///
    /// # Panics
    /// If `seeds` is not one seed per env.
    pub fn restart(&mut self, seeds: &[u64], next: u64) {
        assert_eq!(seeds.len(), self.batch.len(), "one seed per env");
        for (k, (env, &seed)) in (0..).zip(self.batch.iter_mut().zip(seeds)) {
            env.episode = Some((k, seed));
            env.start = None;
            env.fresh = true;
        }
        self.next = next;
        self.clear_ended();
        self.reset_fresh();
        self.write_cells();
    }

    /// Makes env `dst` a copy of env `src`'s position: the run (its streams included), the
    /// episode and its counters, then the observation and mask as a step writes them. Played
    /// with the same actions, `dst` ends as `src` would. For lookahead (RLN27); a live env
    /// (`src` has an episode) is the only meaningful source.
    ///
    /// With `reseed`, a copy in a fight then hides what the player can't know
    /// ([`sts2_core::Run::determinize_combat`]: the draw order and the fight's random streams
    /// reseeded from that seed), so the copy plays a plausible future, not the real one (RLN28).
    /// Outside a fight it is an exact copy either way.
    ///
    /// # Panics
    /// If an index is out of range, or `reseed` is given with `src == dst`.
    pub fn copy_env(&mut self, src: usize, dst: usize, reseed: Option<u64>) {
        if src == dst {
            assert!(reseed.is_none(), "a reseeded copy needs a separate slot");
            return;
        }
        let (from, env) = if src < dst {
            let (a, b) = self.batch.split_at_mut(dst);
            (&a[src], &mut b[0])
        } else {
            let (a, b) = self.batch.split_at_mut(src);
            (&b[0], &mut a[dst])
        };
        env.copy_from(from, reseed);
        self.done[dst] = 0;
        let mut slots = Slots {
            obs: &mut self.obs[dst * RUN_OBS_BYTES..][..RUN_OBS_BYTES],
            privs: &mut self.privs[dst * PRIV_BYTES..][..PRIV_BYTES],
            mask: &mut self.masks[dst * RUN_MASK_WORDS..][..RUN_MASK_WORDS],
            counts: &mut self.class_counts[dst * RUN_ACTIONS..][..RUN_ACTIONS],
        };
        env.settle_copy(&mut slots);
        self.cells[dst] = cell_code(&self.batch[dst]);
    }

    /// [`RunEnvs::step_with`] without looking at the finished episodes.
    ///
    /// # Errors
    /// As [`RunEnvs::step_with`].
    pub fn step(&mut self, actions: &[u16]) -> Result<RunStepStats, RunStepError> {
        self.step_with(actions, |_| {})
    }

    /// Applies `actions[i]` (an index of the run space, or [`HOLD`]) to env `i` for every env still playing,
    /// plays on to each env's next decision, reports each episode that ended to `on_end` in env
    /// order and resets its env.
    ///
    /// # Errors
    /// If the batch has the wrong length or a live env's action is not in its mask; nothing
    /// is applied then.
    pub fn step_with(
        &mut self,
        actions: &[u16],
        mut on_end: impl FnMut(&RunEpisodeEnd<'_>),
    ) -> Result<RunStepStats, RunStepError> {
        if actions.len() != self.batch.len() {
            return Err(RunStepError::Length {
                expected: self.batch.len(),
                got: actions.len(),
            });
        }
        for (env, (e, &action)) in self.batch.iter().zip(actions).enumerate() {
            if action != HOLD && e.acts() && !e.mask.contains(usize::from(action)) {
                return Err(RunStepError::Illegal { env, action });
            }
        }
        let counts = each_env(
            self.threads,
            &mut self.batch,
            Buffers {
                obs: &mut self.obs,
                privs: &mut self.privs,
                masks: &mut self.masks,
                counts: &mut self.class_counts,
            },
            |i, env, slots| env.step(actions[i], slots),
        );
        if self.config.options.events {
            for (out, env) in self.events.iter_mut().zip(&self.batch) {
                *out = env.events;
            }
        }
        let mut stats = RunStepStats::default();
        let mut counts = counts;
        if self.practice.is_some() {
            self.collect_snapshots();
        }
        // Ended episodes, then their envs' next ones; an episode that ends at its reset
        // (`Run::new` failed, or no decision before the end) ends in the next step.
        for i in 0..self.batch.len() {
            (self.done[i], self.won[i], self.ends[i]) = (0, 0, 0);
            (self.floors[i], self.ended_cells[i]) = (0, 0);
            let env = &mut self.batch[i];
            if env.end == RunEnd::None {
                continue;
            }
            // Keep the ended episode's position before `assign`/`reset_fresh` overwrites it.
            self.final_obs[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES]
                .copy_from_slice(&self.obs[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES]);
            self.final_masks[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]
                .copy_from_slice(&self.masks[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]);
            if self.config.options.priv_obs {
                self.final_privs[i * PRIV_BYTES..][..PRIV_BYTES]
                    .copy_from_slice(&self.privs[i * PRIV_BYTES..][..PRIV_BYTES]);
            }
            self.ended_cells[i] = cell_code(env);
            let Some((episode, seed)) = env.episode else {
                continue;
            };
            let end = RunEpisodeEnd {
                env: i,
                episode,
                seed,
                end: env.end,
                floor: env.floor(),
                actions: env.actions,
                decisions: env.decisions,
                auto: env.auto,
                resolved: env.resolved,
                bosses: env.bosses,
                run: env.run.as_ref(),
                cell: env.segment.map(|(cell, _)| cell),
                log: env.log.as_deref().unwrap_or_default(),
            };
            on_end(&end);
            stats.ended += 1;
            stats.won += u32::from(end.end == RunEnd::Victory);
            stats.capped += u32::from(end.end.is_cap());
            stats.fight_capped += u32::from(end.end == RunEnd::FightCapped);
            stats.decision_capped += u32::from(end.end == RunEnd::DecisionCapped);
            stats.stuck += u32::from(end.end == RunEnd::Stuck);
            self.done[i] = 1;
            self.won[i] = u8::from(end.end == RunEnd::Victory);
            self.ends[i] = end.end as u8;
            self.floors[i] = end.floor;
            env.end = RunEnd::None;
            self.assign(i);
            stats.reset += u32::from(self.batch[i].episode.is_some());
        }
        if stats.ended > 0 {
            counts = counts + self.reset_fresh();
            self.write_cells();
        }
        stats.actions = counts.actions;
        stats.decisions = counts.decisions;
        stats.auto = counts.auto;
        stats.resolved = counts.resolved;
        stats.refused = counts.refused;
        stats.unmapped = counts.unmapped;
        stats.merged = counts.merged;
        stats.unmapped_classes = counts.unmapped_classes;
        stats.auto_class = counts.auto_class;
        stats.unmapped_decisions = counts.unmapped_decisions;
        stats.saturated = counts.saturated;
        stats.member_mismatch = counts.member_mismatch;
        Ok(stats)
    }

    /// With [`RunOptions::priv_obs`], the privileged buffers ([`crate::priv_obs`]): env `i`'s is
    /// `priv_obs()[i * PRIV_BYTES..][..PRIV_BYTES]`, for the value head only; zeros when off.
    #[must_use]
    pub fn priv_obs(&self) -> &[u8] {
        &self.privs
    }

    /// The observations: env `i`'s is `obs()[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES]`.
    #[must_use]
    pub fn obs(&self) -> &[u8] {
        &self.obs
    }

    /// The observation of the episode that ended in the last step, per env (the current
    /// observation after the reset is in [`RunEnvs::obs`]). Zeros until an episode ends.
    #[must_use]
    pub fn final_obs(&self) -> &[u8] {
        &self.final_obs
    }

    /// The mask of the episode that ended in the last step, per env (as [`RunEnvs::masks`]).
    #[must_use]
    pub fn final_masks(&self) -> &[u64] {
        &self.final_masks
    }

    /// With [`RunOptions::priv_obs`], the privileged buffer of the episode that ended in the
    /// last step, per env (as [`RunEnvs::priv_obs`]); for the value head only.
    #[must_use]
    pub fn final_priv(&self) -> &[u8] {
        &self.final_privs
    }

    /// Per env, the cell code of its episode: 0 a normal run, else the practice cell's
    /// [`Cell::index`] + 1.
    #[must_use]
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    /// Per env, the cell code (as [`RunEnvs::cells`]) of the episode that ended in the last
    /// step; 0 for the others.
    #[must_use]
    pub fn ended_cells(&self) -> &[u8] {
        &self.ended_cells
    }

    /// The masks: env `i`'s is `masks()[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]` (all zero when
    /// the env has no episode).
    #[must_use]
    pub fn masks(&self) -> &[u64] {
        &self.masks
    }

    /// Env `i`'s mask.
    #[must_use]
    pub fn mask(&self, i: usize) -> &[u64] {
        &self.masks[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]
    }

    /// With [`RunOptions::dedupe`], the member count of each kept index's class: env `i`'s is
    /// `class_counts()[i * RUN_ACTIONS..][..RUN_ACTIONS]`, 0 at every index not in its mask.
    /// All zero without the option.
    #[must_use]
    pub fn class_counts(&self) -> &[u8] {
        &self.class_counts
    }

    /// Env `i`'s class counts.
    #[must_use]
    pub fn class_count(&self, i: usize) -> &[u8] {
        &self.class_counts[i * RUN_ACTIONS..][..RUN_ACTIONS]
    }

    /// With [`RunOptions::events`], per env, what happened to its episode in the last step
    /// (the episode that ended, for an env that was reset). All zero without the option.
    #[must_use]
    pub fn events(&self) -> &[RunEvents] {
        &self.events
    }

    /// Per env, 1 when its episode ended in the last step (the env was reset since).
    #[must_use]
    pub fn done(&self) -> &[u8] {
        &self.done
    }

    /// Per env, 1 when its episode ended in the last step with a victory.
    #[must_use]
    pub fn won(&self) -> &[u8] {
        &self.won
    }

    /// Per env, how the episode that ended in the last step ended ([`RunEnd`] as `u8`, 0 if
    /// none did).
    #[must_use]
    pub fn ends(&self) -> &[u8] {
        &self.ends
    }

    /// Per env, the total floor the episode that ended in the last step reached.
    #[must_use]
    pub fn floors(&self) -> &[u16] {
        &self.floors
    }

    /// Env `i`'s episode serial and seed; `None` once the source has no more for it.
    #[must_use]
    pub fn episode(&self, i: usize) -> Option<(u64, u64)> {
        self.batch[i].episode
    }

    /// The number of envs still playing an episode.
    #[must_use]
    pub fn live(&self) -> usize {
        self.batch.iter().filter(|e| e.episode.is_some()).count()
    }

    /// Env `i`'s run, read-only.
    #[must_use]
    pub fn run(&self, i: usize) -> Option<&Run> {
        self.batch[i].run.as_ref()
    }

    /// Env `i`'s current legal list, for checking its mask.
    #[must_use]
    pub fn legal(&self, i: usize) -> &[RunAction] {
        &self.batch[i].legal
    }

    /// The action env `i` plays for mask index `index` (a class's first member), as
    /// [`RunEnvs::step`] maps it.
    #[must_use]
    pub fn action(&self, i: usize, index: usize) -> Option<&RunAction> {
        let env = &self.batch[i];
        env.legal.get(env.mask.position(index)?)
    }
}
