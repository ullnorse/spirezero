//! Engine throughput: random-play fights and whole runs per second, on one or many threads.
//!
//! `cargo xtask bench [--mode fights|runs|both|rl] [--threads N|all] [--seeds N] [--reps N]
//! [--runs N] [--seed-start N] [--character NAME|all] [--lookahead [--fresh-copy]]
//! [--envs N] [--no-obs] [--no-intents] [--no-mask] [--check-mask] [--starts FILE]
//! [--combat agent|greedy|random]` (`--mode run-env`)
//!
//! **Fights** (spire-mentor's "W1", `sts2-rs/crates/sts2-bench/examples/site_random_bench.rs`,
//! so the two engines can be compared like for like): an Ironclad at ascension 0 with the
//! starter deck, 80/80 HP and Burning Blood fights the 22 Overgrowth encounters round-robin (seed
//! `s` fights encounter `s % 22` in `OVERGROWTH.encounters` order: 4 weak, 12 normal, 3 elites, 3
//! bosses). Each seed's start state is built once, untimed; each of its `--reps` fights clones it
//! and plays uniformly random legal actions to the end, timed. An action is one `apply` call.
//!
//! **W1-all**: `--character NAME` plays W1 with that character's starter deck, HP and relic
//! instead (same seeds and encounters; Ironclad's is W1 itself); `--character all` reports W1
//! for each of the five characters in select order, one line each.
//!
//! **RL** (`--mode rl`, [`rl`]): the fights a vectorised RL trainer steps, `--envs N` per thread
//! (default 64), each step writing the legal-action bitset and the observation.
//!
//! **Run env** (`--mode run-env`, [`run_env`]): whole runs as RL episodes, `--envs N` per thread,
//! each step one agent decision over the run space with the mask and run observation written;
//! `--combat` says who plays the fights.
//!
//! **Runs**: a Necrobinder (or `--character`; `all` plays seed `s` as the `s % 5`th character in
//! select order) at ascension 0 plays whole runs from Neow on with uniformly random
//! legal actions (an action `apply` refuses is skipped; at most 40 card plays per turn) until
//! death, victory, unported content, a screen with no accepted action, or 60,000 actions. Timed
//! from `Run::new` on.
//!
//! **Engines** (`--engine`, fights only): `core` is sts2-core. `kernel` plays the same fights on
//! the experimental `sts2-kernel` (PF6): each seed's start state is projected from the sts2-core
//! combat once (untimed) and every fight copies it; the kernel is bit-exact with sts2-core on W1,
//! so `wins`, `actions` and `digest` must equal the core run's. `lockstep` plays the kernel's
//! fights `--lanes N` seeds at a time per thread, one action per lane in turn (same digest).
//!
//! `--lookahead` adds a 1-ply search's work to each fight step: copy the fight and apply each legal
//! action to the copy (the copies are dropped, so the digest does not change). The copy is one
//! scratch fight per seed refilled with `Combat::clone_from`, which reuses its buffers, as a
//! search would. `--fresh-copy` makes each copy a fresh `clone` instead (as before PF12), so
//! lookahead numbers compare with PF11's; the digest is the same either way.
//!
//! Seeds are split across threads by index, and each seed's policy RNG depends only on the
//! seed, so `wins`, `actions` and `digest` do not depend on `--threads`: two builds whose
//! digests differ do different work. The rate of a multi-threaded run is the sum of each
//! thread's own rate (spire-mentor's aggregate rate); the wall-clock rate is printed too.

use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use sts2_core::combat::Action;
use sts2_core::content::encounters::Encounter;
use sts2_core::run::rooms::OVERGROWTH;
use sts2_core::run::{Act1, Run, RunAction, RunDecision, UnlockState};
use sts2_core::{Character, Combat, CombatConfig, Status};
use sts2_kernel::{Action as KernelAction, Actions, Fight, Kernel, Lane, Status as KernelStatus};

mod rl;
mod run_env;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Actions a fight may take before it counts as an anomaly (random play ends far sooner).
const MAX_FIGHT_ACTIONS: u64 = 10_000;
/// Actions a run may take before it counts as capped.
const MAX_RUN_ACTIONS: u64 = 60_000;
/// Card plays in one turn before the run policy only ends the turn (zero-cost loops).
const MAX_PLAYS_PER_TURN: usize = 40;
/// Worker stack: the engine recurses deeply in long fights.
const STACK_BYTES: usize = 64 << 20;

/// `SplitMix64`: the policy's own RNG, never the engine's streams.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Policy(u64);

impl Policy {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn pick(&mut self, len: usize) -> usize {
        let len = u64::try_from(len).expect("usize fits u64");
        usize::try_from(self.next() % len).expect("below len")
    }
}

/// FNV-1a over a few words: one fight's or run's contribution to the digest.
pub(crate) fn fnv(words: &[u64]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325_u64;
    for w in words {
        for b in w.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }
    h
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Fights,
    Runs,
    Both,
    Rl,
    RunEnv,
}

/// Which characters `--character` asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Chars {
    /// None: W1 is Ironclad, runs are Necrobinder, RL cycles through all five by seed.
    Default,
    One(Character),
    /// Fights report each character; runs and RL cycle through them by seed.
    All,
}

/// Which engine plays the fights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Engine {
    Core,
    Kernel,
    Lockstep,
}

#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per independent bench option"
)]
pub(crate) struct Config {
    mode: Mode,
    engine: Engine,
    lanes: usize,
    /// Before each action, copy the fight and apply each legal action to the copy (a 1-ply
    /// search's work; the digest does not change).
    lookahead: bool,
    /// The lookahead's copies are fresh clones rather than one scratch fight refilled with
    /// `clone_from`.
    fresh_copy: bool,
    threads: usize,
    seeds: u64,
    seed_start: u64,
    reps: u64,
    runs: u64,
    characters: Chars,
    /// The fights' character (one of `characters`, set per report).
    fight_character: Character,
    /// RL: envs per thread; whether a step writes the mask, the observation, and the intents;
    /// whether every step checks the mask against the legal list.
    envs: usize,
    mask: bool,
    obs: bool,
    intents: bool,
    check_mask: bool,
    /// RL: a start corpus (JSON lines) instead of W1-all's starts.
    starts: Option<String>,
    /// Run env: who plays the fights.
    combat: sts2_rl::CombatMode,
}

impl Config {
    /// Takes `flag` if it is one of the RL mode's presence flags.
    fn rl_flag(&mut self, flag: &str) -> bool {
        match flag {
            "--no-mask" => {
                self.mask = false;
                self.obs = false;
            }
            "--no-obs" => self.obs = false,
            "--no-intents" => self.intents = false,
            "--check-mask" => self.check_mask = true,
            _ => return false,
        }
        true
    }

    fn parse(args: &[String]) -> Result<Self, String> {
        let mut config = Self {
            mode: Mode::Both,
            engine: Engine::Core,
            lanes: 16,
            lookahead: false,
            fresh_copy: false,
            threads: 1,
            seeds: 2200,
            seed_start: 0,
            reps: 50,
            runs: 200,
            characters: Chars::Default,
            fight_character: Character::Ironclad,
            envs: 64,
            mask: true,
            obs: true,
            intents: true,
            check_mask: false,
            starts: None,
            combat: sts2_rl::CombatMode::Agent,
        };
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            if flag == "--lookahead" {
                config.lookahead = true;
                continue;
            }
            if flag == "--fresh-copy" {
                config.fresh_copy = true;
                continue;
            }
            if config.rl_flag(flag) {
                continue;
            }
            let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
            let number = || value.parse::<u64>().map_err(|e| format!("{flag}: {e}"));
            match flag.as_str() {
                "--mode" => {
                    config.mode = match value.as_str() {
                        "fights" => Mode::Fights,
                        "runs" => Mode::Runs,
                        "both" => Mode::Both,
                        "rl" => Mode::Rl,
                        "run-env" => Mode::RunEnv,
                        _ => return Err(format!("unknown mode {value}")),
                    }
                }
                "--threads" if value == "all" => {
                    config.threads = thread::available_parallelism().map_or(1, usize::from);
                }
                "--threads" => {
                    config.threads = usize::try_from(number()?).map_err(|e| e.to_string())?;
                }
                "--engine" => {
                    config.engine = match value.as_str() {
                        "core" => Engine::Core,
                        "kernel" => Engine::Kernel,
                        "lockstep" => Engine::Lockstep,
                        _ => return Err(format!("unknown engine {value}")),
                    }
                }
                "--lanes" => {
                    config.lanes = usize::try_from(number()?).map_err(|e| e.to_string())?;
                }
                "--starts" => config.starts = Some(value.clone()),
                "--combat" => {
                    config.combat = sts2_rl::CombatMode::parse(value)
                        .ok_or_else(|| format!("unknown combat mode {value}"))?;
                }
                "--seeds" => config.seeds = number()?,
                "--seed-start" => config.seed_start = number()?,
                "--reps" => config.reps = number()?,
                "--runs" => config.runs = number()?,
                "--envs" => {
                    config.envs = usize::try_from(number()?).map_err(|e| e.to_string())?;
                }
                "--character" if value == "all" => config.characters = Chars::All,
                "--character" => {
                    config.characters = Chars::One(
                        Character::from_name(value)
                            .ok_or_else(|| format!("unknown character {value}"))?,
                    );
                }
                _ => return Err(format!("unknown flag {flag}")),
            }
        }
        config.threads = config.threads.max(1);
        config.lanes = config.lanes.max(1);
        config.envs = config.envs.max(1);
        Ok(config)
    }
}

/// What one thread (or the sum of all) did.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Tally {
    items: u64,
    wins: u64,
    actions: u64,
    /// Fights over the action cap; runs that stopped on unported content, got stuck or were capped.
    anomalies: u64,
    /// RL: legal actions the fixed action space has no index for.
    unmapped: u64,
    /// Run env: engine actions (the agent's, automatic and resolved), of those automatic and
    /// resolved, and the floors the runs reached, summed.
    engine_actions: u64,
    auto: u64,
    resolved: u64,
    floors: u64,
    digest: u64,
    timed: Duration,
}

impl Tally {
    fn add(&mut self, other: &Self) {
        self.items += other.items;
        self.wins += other.wins;
        self.actions += other.actions;
        self.anomalies += other.anomalies;
        self.unmapped += other.unmapped;
        self.engine_actions += other.engine_actions;
        self.auto += other.auto;
        self.resolved += other.resolved;
        self.floors += other.floors;
        self.digest = self.digest.wrapping_add(other.digest);
        self.timed += other.timed;
    }
}

/// `count` per second over `elapsed`, in integers (no floats in this workspace).
fn per_second(count: u64, elapsed: Duration) -> u128 {
    u128::from(count) * 1_000_000 / elapsed.as_micros().max(1)
}

/// Seed `s`'s fight start state for `character` (built untimed).
pub(crate) fn fight_start(character: Character, seed: u64) -> Combat {
    let info = &OVERGROWTH.encounters
        [usize::try_from(seed).expect("fits usize") % OVERGROWTH.encounters.len()];
    let encounter = Encounter::from_entry(info.id).expect("every Overgrowth encounter is ported");
    let config = CombatConfig::new(character, &format!("W1S{seed}"), 0)
        .expect("every starter card is ported")
        .with_encounter(encounter);
    Combat::new(config)
}

/// The fight workload for the seeds `t, t + threads, ...` of the range.
fn fights(config: &Config, t: usize) -> Tally {
    let mut tally = Tally::default();
    let first = config.seed_start + u64::try_from(t).expect("fits u64");
    for seed in (first..config.seed_start + config.seeds).step_by(config.threads) {
        let start = fight_start(config.fight_character, seed);
        let mut policy = Policy(fnv(&[seed]));
        let clock = Instant::now();
        let mut legal = Vec::new();
        // The lookahead's copy: refilled with `clone_from`, which keeps its buffers.
        let mut copy = start.clone();
        for rep in 0..config.reps {
            let mut combat = start.clone();
            let mut actions = 0;
            while matches!(combat.status(), Status::PlayerTurn | Status::Decision)
                && actions < MAX_FIGHT_ACTIONS
            {
                combat.legal_actions_into(&mut legal);
                if config.lookahead {
                    for a in &legal {
                        if config.fresh_copy {
                            let mut fresh = combat.clone();
                            fresh.apply(a.clone()).expect("legal action");
                            std::hint::black_box(&fresh);
                        } else {
                            copy.clone_from(&combat);
                            copy.apply(a.clone()).expect("legal action");
                            std::hint::black_box(&copy);
                        }
                    }
                }
                let action = legal.swap_remove(policy.pick(legal.len()));
                combat.apply(action).expect("legal action");
                actions += 1;
            }
            let won = combat.status() == Status::Won;
            tally.items += 1;
            tally.wins += u64::from(won);
            tally.actions += actions;
            tally.anomalies += u64::from(actions >= MAX_FIGHT_ACTIONS);
            tally.digest = tally
                .digest
                .wrapping_add(fnv(&[seed, rep, u64::from(won), actions]));
        }
        tally.timed += clock.elapsed();
    }
    tally
}

/// Seed `s`'s fight start state on the kernel (built untimed).
fn kernel_start(k: &Kernel, seed: u64) -> Fight {
    Fight::from_core(k, &fight_start(Character::Ironclad, seed))
        .expect("W1 is in the kernel's scope")
}

/// One fight's digest words and outcome, shared by the kernel engines.
fn kernel_outcome(tally: &mut Tally, seed: u64, rep: u64, fight: &Fight, actions: u64) {
    let won = fight.status == KernelStatus::Won;
    tally.items += 1;
    tally.wins += u64::from(won);
    tally.actions += actions;
    tally.anomalies +=
        u64::from(actions >= MAX_FIGHT_ACTIONS || fight.status == KernelStatus::Unsupported);
    tally.digest = tally
        .digest
        .wrapping_add(fnv(&[seed, rep, u64::from(won), actions]));
}

/// [`fights`] on the kernel.
fn kernel_fights(config: &Config, t: usize) -> Tally {
    let k = Kernel::get();
    let mut tally = Tally::default();
    let first = config.seed_start + u64::try_from(t).expect("fits u64");
    for seed in (first..config.seed_start + config.seeds).step_by(config.threads) {
        let start = kernel_start(k, seed);
        let mut policy = Policy(fnv(&[seed]));
        let mut legal = Actions::new();
        let clock = Instant::now();
        for rep in 0..config.reps {
            let mut fight = start;
            let mut actions = 0;
            while matches!(
                fight.status,
                KernelStatus::PlayerTurn | KernelStatus::Decision
            ) && actions < MAX_FIGHT_ACTIONS
            {
                fight.legal_actions_into(k, &mut legal);
                if config.lookahead {
                    for &a in legal.as_slice() {
                        let mut copy = fight;
                        copy.step_unchecked(k, a);
                        std::hint::black_box(&copy);
                    }
                }
                let action = legal.as_slice()[policy.pick(legal.len())];
                fight.step_unchecked(k, action);
                actions += 1;
            }
            kernel_outcome(&mut tally, seed, rep, &fight, actions);
        }
        tally.timed += clock.elapsed();
    }
    tally
}

/// A lockstep lane: one seed's fights, then the next seed of its share of the queue.
struct BenchLane<'a> {
    starts: &'a [(u64, Fight)],
    /// The lane's next queue index, and the step between its seeds.
    next: usize,
    stride: usize,
    seed: u64,
    start: Fight,
    fight: Fight,
    rep: u64,
    reps: u64,
    policy: Policy,
    tally: Tally,
}

impl BenchLane<'_> {
    /// Loads the lane's next seed; false when its share of the queue is done.
    fn load(&mut self) -> bool {
        let Some(&(seed, start)) = self.starts.get(self.next) else {
            return false;
        };
        self.next += self.stride;
        self.seed = seed;
        self.start = start;
        self.fight = start;
        self.rep = 0;
        self.policy = Policy(fnv(&[seed]));
        true
    }
}

impl Lane for BenchLane<'_> {
    fn fight(&mut self) -> &mut Fight {
        &mut self.fight
    }

    fn pick(&mut self, legal: &[KernelAction]) -> KernelAction {
        legal[self.policy.pick(legal.len())]
    }

    fn finished(&mut self, actions: u64) -> bool {
        kernel_outcome(&mut self.tally, self.seed, self.rep, &self.fight, actions);
        self.rep += 1;
        if self.rep < self.reps {
            self.fight = self.start;
            true
        } else {
            self.load()
        }
    }
}

/// [`fights`] on the kernel, `config.lanes` seeds in lockstep at a time. The start states are
/// built before the clock starts.
fn lockstep_fights(config: &Config, t: usize) -> Tally {
    let k = Kernel::get();
    let first = config.seed_start + u64::try_from(t).expect("fits u64");
    let starts: Vec<(u64, Fight)> = (first..config.seed_start + config.seeds)
        .step_by(config.threads)
        .map(|seed| (seed, kernel_start(k, seed)))
        .collect();
    let Some(&(_, any)) = starts.first() else {
        return Tally::default();
    };
    let lanes = config.lanes.min(starts.len());
    let mut pool: Vec<BenchLane<'_>> = (0..lanes)
        .map(|lane| {
            let mut l = BenchLane {
                starts: &starts,
                next: lane,
                stride: lanes,
                seed: 0,
                start: any,
                fight: any,
                rep: 0,
                reps: config.reps,
                policy: Policy(0),
                tally: Tally::default(),
            };
            l.load();
            l
        })
        .collect();
    let clock = Instant::now();
    if config.reps > 0 {
        sts2_kernel::run_lockstep(k, &mut pool, MAX_FIGHT_ACTIONS);
    }
    let mut total = Tally::default();
    for lane in &pool {
        total.add(&lane.tally);
    }
    total.timed = clock.elapsed();
    total
}

/// How a run ended: 0 death, 1 victory, 2 unported content, 3 stuck, 4 capped.
fn play_run(seed: u64, character: Option<Character>) -> (u64, u64) {
    let character =
        character.unwrap_or_else(|| Character::ALL[usize::try_from(seed % 5).expect("below 5")]);
    let mut run = Run::new(
        character,
        &format!("RUN{seed}"),
        0,
        UnlockState::all(),
        Act1::Random,
    )
    .expect("a valid run config");
    let mut policy = Policy(fnv(&[seed, 1]));
    let mut actions = 0;
    let mut plays = 0;
    loop {
        match run.pending_decision() {
            RunDecision::GameOver => return (0, actions),
            RunDecision::Victory => return (1, actions),
            RunDecision::Stopped(_) => return (2, actions),
            _ if actions >= MAX_RUN_ACTIONS => return (4, actions),
            _ => {}
        }
        let mut legal = run.legal_actions();
        if plays >= MAX_PLAYS_PER_TURN {
            legal.retain(|a| !matches!(a, RunAction::Combat(Action::PlayCard { .. })));
        }
        let mut applied = None;
        while !legal.is_empty() {
            let action = legal.swap_remove(policy.pick(legal.len()));
            if run.apply(action.clone()).is_ok() {
                applied = Some(action);
                break;
            }
        }
        let Some(action) = applied else {
            return (3, actions);
        };
        actions += 1;
        plays = match action {
            RunAction::Combat(Action::PlayCard { .. }) => plays + 1,
            RunAction::Combat(Action::UsePotion { .. } | Action::Resolve(_)) => plays,
            _ => 0,
        };
    }
}

/// The run workload for the runs `t, t + threads, ...`.
fn runs(config: &Config, t: usize) -> Tally {
    let mut tally = Tally::default();
    let first = config.seed_start + u64::try_from(t).expect("fits u64");
    for seed in (first..config.seed_start + config.runs).step_by(config.threads) {
        let clock = Instant::now();
        let character = match config.characters {
            Chars::Default => Some(Character::Necrobinder),
            Chars::One(c) => Some(c),
            Chars::All => None,
        };
        let (end, actions) = play_run(seed, character);
        tally.timed += clock.elapsed();
        tally.items += 1;
        tally.wins += u64::from(end == 1);
        tally.anomalies += u64::from(end >= 2);
        tally.actions += actions;
        tally.digest = tally.digest.wrapping_add(fnv(&[seed, end, actions]));
    }
    tally
}

/// Runs `work` on `config.threads` threads; the total, the sum of per-thread rates (items and
/// actions per second) and the wall time.
fn measure(config: &Config, work: fn(&Config, usize) -> Tally) -> (Tally, u128, u128, Duration) {
    let clock = Instant::now();
    let tallies: Vec<Tally> = thread::scope(|scope| {
        #[allow(
            clippy::needless_collect,
            reason = "every worker must be spawned before the first join"
        )]
        let handles: Vec<_> = (0..config.threads)
            .map(|t| {
                thread::Builder::new()
                    .stack_size(STACK_BYTES)
                    .spawn_scoped(scope, move || work(config, t))
                    .expect("spawn a worker")
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker panicked"))
            .collect()
    });
    let wall = clock.elapsed();
    let mut total = Tally::default();
    let (mut items, mut actions) = (0, 0);
    for tally in &tallies {
        total.add(tally);
        items += per_second(tally.items, tally.timed);
        actions += per_second(tally.actions, tally.timed);
    }
    (total, items, actions, wall)
}

#[allow(
    clippy::print_stdout,
    reason = "the benchmark's output is its interface"
)]
fn report(label: &str, config: &Config, work: fn(&Config, usize) -> Tally) {
    let (total, items, actions, wall) = measure(config, work);
    let ns = u128::try_from(config.threads).expect("fits u128") * 1_000_000_000 / actions.max(1);
    // W1 keeps its line; W1-all names the character, RL its envs and extraction.
    let extra = match (config.mode, config.characters) {
        (Mode::Rl, _) => format!(
            " envs={} mask={} obs={} intents={} obs_bytes/step={} unmapped={}",
            config.envs,
            config.mask,
            config.obs,
            config.obs && config.intents,
            if config.obs { rl::OBS_BYTES } else { 0 },
            total.unmapped,
        ),
        (Mode::RunEnv, _) => format!(
            " envs={} combat={:?} obs_bytes/step={} engine_actions={} auto={} resolved={} \
             floors={} unmapped={}",
            config.envs,
            config.combat,
            sts2_rl::RUN_OBS_BYTES,
            total.engine_actions,
            total.auto,
            total.resolved,
            total.floors,
            total.unmapped,
        ),
        (_, Chars::Default) => String::new(),
        _ => format!(" character={}", config.fight_character.name()),
    };
    println!(
        "{label}: threads={}{extra} {label}={} wins={} actions={} anomalies={} digest={:016x} | \
         {label}/s={items} actions/s={actions} ns/action/thread={ns} | wall {} ms: {label}/s={} actions/s={}",
        config.threads,
        total.items,
        total.wins,
        total.actions,
        total.anomalies,
        total.digest,
        wall.as_millis(),
        per_second(total.items, wall),
        per_second(total.actions, wall),
    );
}

#[allow(clippy::print_stderr, reason = "usage errors go to stderr")]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match Config::parse(&args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("sts2-bench: {message}");
            return ExitCode::FAILURE;
        }
    };
    if config.mode == Mode::Rl {
        report("rl-fights", &config, rl::rl);
        return ExitCode::SUCCESS;
    }
    if config.mode == Mode::RunEnv {
        report("rl-runs", &config, run_env::run_env);
        return ExitCode::SUCCESS;
    }
    if config.mode != Mode::Runs {
        let work = match config.engine {
            Engine::Core => fights,
            Engine::Kernel => kernel_fights,
            Engine::Lockstep => lockstep_fights,
        };
        let characters: &[Character] = match config.characters {
            Chars::Default => &[Character::Ironclad],
            Chars::One(ref c) => std::slice::from_ref(c),
            Chars::All => &Character::ALL,
        };
        for &fight_character in characters {
            let config = Config {
                fight_character,
                ..config.clone()
            };
            report("fights", &config, work);
        }
    }
    if config.mode != Mode::Fights {
        report("runs", &config, runs);
    }
    ExitCode::SUCCESS
}
