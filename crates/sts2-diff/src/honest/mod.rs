//! The honest bot (RLB6): whole runs played from what the player sees (`docs/specs/honesty.md`).
//!
//! [`Bot::choose`] gets the run's state and pending decision and returns one of the legal
//! actions. Out of combat it uses the visible heuristics of [`run`] and the measured option
//! values of [`values`]; in combat a [`Controller`]: the combat PPO net ([`ppo`]) on the fight's
//! leak-tested observation, or the determinized search ([`search`]) over blindfolded worlds.
//! The action-leak test (`tests/it/honest.rs`) checks that two states that differ only in what
//! is hidden get the same action.
//!
//! [`play`] plays a seed, [`play_batch`] a seed list on threads; a win counts only once
//! [`crate::reach::replay`] has replayed its actions from the seed to victory.

pub mod advice;
pub mod az;
pub mod log;
pub mod policy;
pub mod ppo;
pub mod probe;
pub mod rollout;
pub mod run;
pub mod search;
mod table;
pub mod tree;
pub mod value;
pub mod values;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Instant;

use sts2_core::Character;
use sts2_core::combat::Action;
use sts2_core::run::{Act1, Room, RoomType, Run, RunAction, RunDecision, RunState, UnlockState};

use self::search::{FightContext, SearchConfig};

/// A fight that runs past this many player turns is a stalemate (a policy guard).
const MAX_COMBAT_TURNS: u32 = 60;
/// The engine actions a run may take (as the run env's cap).
pub const MAX_ACTIONS: usize = 60_000;
/// A fight that takes more actions than this is a stalemate too (a time bound for runs the
/// learned policy steers into odd decks; RLB8).
const MAX_FIGHT_ACTIONS: usize = 1_500;

/// What plays the fights.
#[derive(Clone, Debug)]
pub enum Controller {
    /// The combat PPO net, greedy.
    Ppo(Arc<ppo::Net>),
    /// The determinized search (RLB6).
    Search(SearchConfig),
    /// The determinized search with the learned playout policy.
    SearchRollout(SearchConfig, Arc<rollout::RolloutNet>),
    /// The chance-node tree search (RLB7), its playouts by the learned policy when it has one.
    Tree(tree::TreeConfig, Option<Arc<rollout::RolloutNet>>),
    /// The determinized search with the value net (RLB11): its leaves scored by the net
    /// ([`search::ValueUse::leaves`]), else its playouts by the net's policy; with a learned
    /// playout policy, the leaves mixed with its playouts ([`search::ValueUse::mix`], RLB14).
    SearchValue(
        SearchConfig,
        Arc<value::ValueNet>,
        Option<Arc<rollout::RolloutNet>>,
    ),
    /// The `AlphaZero`-style tree (RLB13) in the fights of its scope, the value search elsewhere.
    Az(az::AzConfig, SearchConfig, Arc<value::ValueNet>),
}

/// The honest bot.
#[derive(Clone, Debug)]
pub struct Bot {
    pub controller: Controller,
    /// The combat net, for the search's playouts.
    pub net: Option<Arc<ppo::Net>>,
    /// The learned run policy (RLB8); the hand rules of [`run`] without one.
    pub run_policy: Option<Arc<policy::RunNet>>,
}

impl Bot {
    /// The bot's action among `legal` (non-empty: the run's legal actions it accepts) for a run
    /// in state `s` facing `decision`. It reads only what the player sees.
    #[must_use]
    pub fn choose(
        &self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
    ) -> RunAction {
        let RunDecision::Combat(combat) = decision else {
            if CURSE_REMOVAL.load(std::sync::atomic::Ordering::Relaxed)
                && let Some(action) = run::curse_removal(s, decision, legal)
            {
                return action;
            }
            return self
                .run_policy
                .as_ref()
                .and_then(|net| probe::choose(net, s, decision, legal))
                .unwrap_or_else(|| run::choose(s, decision, legal));
        };
        let fight: Vec<Action> = legal
            .iter()
            .filter_map(|a| match a {
                RunAction::Combat(a) => Some(a.clone()),
                _ => None,
            })
            .collect();
        if fight.is_empty() {
            return legal[0].clone();
        }
        let action = self.fight_action(combat, &fight, context_of(s));
        RunAction::Combat(action)
    }
}

impl Bot {
    /// The value search's scores at this decision (RLB17), or the controller's move when it is
    /// not the value search or the move is forced.
    #[must_use]
    pub fn fight_scored(
        &self,
        combat: &sts2_core::Combat,
        fight: &[Action],
        context: FightContext,
    ) -> search::Searched {
        let Controller::SearchValue(config, net, rollout) = &self.controller else {
            return search::Searched::Forced(self.fight_action(combat, fight, context));
        };
        let mut config = *config;
        if let (true, 2, Some(hp)) = (context.boss, context.act, config.value.last_boss_hp) {
            config.value.hp = hp;
        }
        let policy = rollout.as_deref().map_or(search::Playout::Value(net), |r| {
            search::Playout::Mixed(net, r)
        });
        search::scored_with(combat, fight, context, config, policy)
    }

    /// The controller's action among `fight` (the fight's legal actions, non-empty).
    #[must_use]
    pub fn fight_action(
        &self,
        combat: &sts2_core::Combat,
        fight: &[Action],
        context: FightContext,
    ) -> Action {
        match &self.controller {
            Controller::Ppo(net) => net
                .choose(combat, fight)
                .or_else(|| search::answer_choice(combat))
                .unwrap_or_else(|| fight[0].clone()),
            Controller::Search(config) => {
                search::choose(combat, fight, context, *config, self.net.as_deref())
            }
            Controller::SearchValue(config, net, rollout) => {
                let mut config = *config;
                if let (true, 2, Some(hp)) = (context.boss, context.act, config.value.last_boss_hp)
                {
                    config.value.hp = hp;
                }
                let policy = rollout.as_deref().map_or(search::Playout::Value(net), |r| {
                    search::Playout::Mixed(net, r)
                });
                search::choose_with(combat, fight, context, config, policy)
            }
            Controller::Az(config, _, net) if config.scope.holds(context) => {
                az::choose(combat, fight, context, *config, net)
            }
            Controller::Az(_, search, net) => {
                search::choose_with(combat, fight, context, *search, search::Playout::Value(net))
            }
            Controller::SearchRollout(config, net) => search::choose_with(
                combat,
                fight,
                context,
                *config,
                search::Playout::Learned(net),
            ),
            Controller::Tree(config, net) if config.mode == tree::Mode::Plans => {
                tree::choose_plans(
                    combat,
                    fight,
                    context,
                    *config,
                    SearchConfig::default(),
                    net.as_deref()
                        .map_or(search::Playout::Priority, search::Playout::Learned),
                )
            }
            Controller::Tree(config, net) if config.hybrid && !context.big => search::choose_with(
                combat,
                fight,
                context,
                SearchConfig::default(),
                net.as_deref()
                    .map_or(search::Playout::Priority, search::Playout::Learned),
            ),
            Controller::Tree(config, net) => tree::choose(
                combat,
                fight,
                context,
                *config,
                net.as_deref()
                    .map_or(search::Playout::Priority, search::Playout::Learned),
            ),
        }
    }
}

/// The fight's context the run shows.
fn context_of(s: &RunState) -> FightContext {
    let room_type = match s.room {
        Room::Combat { room_type, .. } => Some(room_type),
        _ => None,
    };
    let boss = room_type == Some(RoomType::Boss);
    FightContext {
        big: boss || room_type == Some(RoomType::Elite),
        boss,
        act: s.act_index,
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum End {
    Victory,
    Death,
    /// Content the engine does not port.
    Unsupported,
    /// No accepted action.
    Stuck,
    /// The action cap or a stalemated fight.
    Capped,
}

impl End {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Victory => "victory",
            Self::Death => "death",
            Self::Unsupported => "unsupported",
            Self::Stuck => "stuck",
            Self::Capped => "capped",
        }
    }
}

/// RLB16 (`bot --remove-curses`): a shop's card removal goes to a curse first, over the run
/// policy ([`run::curse_removal`]).
pub static CURSE_REMOVAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One played run.
#[derive(Clone, Debug)]
pub struct Played {
    pub seed: u64,
    pub end: End,
    pub act: usize,
    pub floor: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub deck: usize,
    pub relics: usize,
    /// The encounter, event or ancient the run ended in (`""` elsewhere).
    pub encounter: &'static str,
    /// The HP the last fight began with.
    pub fight_hp: i32,
    pub actions: Vec<RunAction>,
    pub millis: u128,
}

/// The game seed of seed number `seed` (`docs/specs/a10-training.md`).
#[must_use]
pub fn seed_name(seed: u64) -> String {
    format!("RUN{seed}")
}

/// A new run as the run env starts it.
///
/// # Errors
/// As `Run::new`.
pub fn new_run(character: Character, seed: u64, ascension: u8) -> Result<Run, String> {
    Run::new(
        character,
        &seed_name(seed),
        ascension,
        UnlockState::all(),
        Act1::Random,
    )
    .map_err(|e| e.to_string())
}

/// The run's end, if it has one.
fn terminal(run: &Run, actions: usize) -> Option<End> {
    match run.pending_decision() {
        RunDecision::Victory => Some(End::Victory),
        RunDecision::GameOver => Some(End::Death),
        RunDecision::Stopped(_) | RunDecision::Unsupported(_) => Some(End::Unsupported),
        _ if actions >= MAX_ACTIONS => Some(End::Capped),
        _ if run.combat().is_some_and(|c| {
            c.state().player.turn_number > MAX_COMBAT_TURNS
                || c.state().cards.len() > search::MAX_CARDS
        }) =>
        {
            Some(End::Capped)
        }
        _ => None,
    }
}

/// Chooses and applies one action; `None` when the run accepts none of its legal actions.
fn step(run: &mut Run, bot: &Bot) -> Option<RunAction> {
    let mut legal = run.legal_actions();
    loop {
        if legal.is_empty() {
            return None;
        }
        let action = bot.choose(run.state(), &run.pending_decision(), &legal);
        if run.apply(action.clone()).is_ok() {
            return Some(action);
        }
        legal.retain(|a| *a != action);
    }
}

/// Plays seed `seed` to its end.
///
/// # Errors
/// If the run cannot be created.
pub fn play(bot: &Bot, character: Character, seed: u64, ascension: u8) -> Result<Played, String> {
    let started = Instant::now();
    let mut run = new_run(character, seed, ascension)?;
    let mut actions = Vec::new();
    let mut fight_hp = 0;
    let mut in_fight = false;
    let mut fight_start = 0;
    let end = loop {
        if let Some(end) = terminal(&run, actions.len()) {
            break end;
        }
        let fighting = run.combat().is_some();
        if fighting && !in_fight {
            fight_hp = run.state().hp;
            fight_start = actions.len();
        }
        in_fight = fighting;
        if fighting && actions.len() - fight_start > MAX_FIGHT_ACTIONS {
            break End::Capped;
        }
        match step(&mut run, bot) {
            Some(action) => actions.push(action),
            None => break End::Stuck,
        }
    };
    Ok(played_of(&run, seed, end, fight_hp, actions, started))
}

/// Plays seed `seed` to its end with the run decisions made by `collector` (sampling from
/// `rng_seed`) and the fights by `bot`; the run and its recorded run decisions.
///
/// # Errors
/// If the run cannot be created.
pub fn collect(
    bot: &Bot,
    collector: policy::Collector<'_>,
    character: Character,
    seed: u64,
    ascension: u8,
    rng_seed: u64,
) -> Result<(Played, Vec<policy::Record>), String> {
    let started = Instant::now();
    let mut rng = policy::SplitMix(rng_seed ^ seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
    let mut run = new_run(character, seed, ascension)?;
    let mut actions = Vec::new();
    let mut records = Vec::new();
    let mut fight_hp = 0;
    let mut in_fight = false;
    let mut fight_start = 0;
    let end = loop {
        if let Some(end) = terminal(&run, actions.len()) {
            break end;
        }
        let fighting = run.combat().is_some();
        if fighting && !in_fight {
            fight_hp = run.state().hp;
            fight_start = actions.len();
        }
        in_fight = fighting;
        if fighting && actions.len() - fight_start > MAX_FIGHT_ACTIONS {
            break End::Capped;
        }
        if fighting {
            match step(&mut run, bot) {
                Some(action) => actions.push(action),
                None => break End::Stuck,
            }
            continue;
        }
        let mut legal = run.legal_actions();
        legal.retain(|a| run.accepts(a));
        if legal.is_empty() {
            break End::Stuck;
        }
        let (action, record) =
            collector.choose(run.state(), &run.pending_decision(), &legal, &mut rng);
        if run.apply(action.clone()).is_ok() {
            actions.push(action);
            records.extend(record);
        } else {
            match step(&mut run, bot) {
                Some(action) => actions.push(action),
                None => break End::Stuck,
            }
        }
    };
    Ok((
        played_of(&run, seed, end, fight_hp, actions, started),
        records,
    ))
}

/// The record of a finished run.
fn played_of(
    run: &Run,
    seed: u64,
    end: End,
    fight_hp: i32,
    actions: Vec<RunAction>,
    started: Instant,
) -> Played {
    let s = run.state();
    Played {
        seed,
        end,
        act: s.act_index + 1,
        floor: s.total_floor(),
        hp: s.hp,
        max_hp: s.max_hp,
        deck: s.deck.len(),
        relics: s.relics.len(),
        encounter: match s.room {
            Room::Combat { encounter, .. } | Room::Event { id: encounter } => encounter,
            Room::Ancient { id } => id,
            _ => "",
        },
        fight_hp,
        actions,
        millis: started.elapsed().as_millis(),
    }
}

/// `f()`, with a panic in the engine as an error, so one run does not end a batch.
///
/// # Errors
/// As `f`, or if it panicked.
pub fn caught<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("the engine panicked".to_owned()))
}

/// Runs `f` on every seed on `jobs` threads; the results in seed order.
///
/// # Panics
/// If a worker panics.
pub fn on_threads<T: Send>(seeds: &[u64], jobs: usize, f: impl Fn(u64) -> T + Sync) -> Vec<T> {
    let jobs = jobs.clamp(1, seeds.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let rows = std::sync::Mutex::new(Vec::with_capacity(seeds.len()));
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn_scoped(scope, || {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&seed) = seeds.get(i) else { break };
                        let row = f(seed);
                        rows.lock().expect("no poisoned lock").push((i, row));
                    }
                })
                .expect("a worker thread");
        }
    });
    let mut rows = rows.into_inner().expect("no poisoned lock");
    rows.sort_by_key(|(i, _)| *i);
    rows.into_iter().map(|(_, row)| row).collect()
}

/// Plays every seed.
#[must_use]
pub fn play_batch(
    bot: &Bot,
    character: Character,
    seeds: &[u64],
    ascension: u8,
    jobs: usize,
) -> Vec<Result<Played, String>> {
    on_threads(seeds, jobs, |seed| play(bot, character, seed, ascension))
}

/// Whether `played` replays from its seed and actions to the same end.
#[must_use]
pub fn replays(played: &Played, character: Character, ascension: u8) -> bool {
    crate::reach::replay(
        character,
        &seed_name(played.seed),
        ascension,
        Act1::Random,
        &played.actions,
    )
    .is_ok_and(|o| {
        (o.end == crate::reach::End::Victory) == (played.end == End::Victory)
            && o.floor == played.floor
    })
}

/// One line per seed.
#[must_use]
pub fn row_line(played: &Played) -> String {
    format!(
        "{}\t{}\tact {}\tfloor {}\thp {}/{}\tfight hp {}\tdeck {}\trelics {}\t{}\tactions {}\tms {}",
        seed_name(played.seed),
        played.end.name(),
        played.act,
        played.floor,
        played.hp,
        played.max_hp,
        played.fight_hp,
        played.deck,
        played.relics,
        if played.encounter.is_empty() {
            "-"
        } else {
            played.encounter
        },
        played.actions.len(),
        played.millis
    )
}

/// The batch summary: wins (replayed), win rate, floors, death floors and killers, actions and
/// time.
#[must_use]
pub fn summary(rows: &[Played], replayed_wins: usize, errors: usize, wall_secs: f64) -> String {
    let n = rows.len().max(1);
    let wins = rows.iter().filter(|r| r.end == End::Victory).count();
    let mut floors: Vec<usize> = rows.iter().map(|r| r.floor).collect();
    floors.sort_unstable();
    let mean = tenths(floors.iter().sum(), n);
    let median = floors.get(floors.len() / 2).copied().unwrap_or(0);
    let mut ends: BTreeMap<&str, usize> = BTreeMap::new();
    let mut killers: BTreeMap<&str, usize> = BTreeMap::new();
    let mut death_floors: BTreeMap<usize, usize> = BTreeMap::new();
    for r in rows {
        *ends.entry(r.end.name()).or_default() += 1;
        if r.end == End::Death {
            *killers.entry(r.encounter).or_default() += 1;
            *death_floors.entry(r.floor).or_default() += 1;
        }
    }
    let mut killers: Vec<_> = killers.into_iter().collect();
    killers.sort_by_key(|(k, c)| (std::cmp::Reverse(*c), *k));
    let actions: usize = rows.iter().map(|r| r.actions.len()).sum();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "runs {} wins {wins} (replayed {replayed_wins}) win rate {} errors {errors}",
        rows.len(),
        thousandths(wins, n)
    );
    let _ = writeln!(out, "floor mean {mean} median {median}");
    let _ = writeln!(out, "ends {ends:?}");
    let _ = writeln!(out, "death floors {death_floors:?}");
    let _ = writeln!(
        out,
        "killers {}",
        killers
            .iter()
            .take(15)
            .map(|(k, c)| format!("{k}:{c}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let _ = writeln!(
        out,
        "actions {actions} (mean {}) wall {wall_secs:.1}s",
        actions / n
    );
    out
}

/// `num / den` to one decimal.
fn tenths(num: usize, den: usize) -> String {
    let x = (num * 10 + den / 2) / den.max(1);
    format!("{}.{}", x / 10, x % 10)
}

/// `num / den` to three decimals.
fn thousandths(num: usize, den: usize) -> String {
    let x = (num * 1000 + den / 2) / den.max(1);
    format!("{}.{:03}", x / 1000, x % 1000)
}

/// The penalty per card an option adds beyond two (a deck clone, a fistful of random cards):
/// the tier list does not price dilution.
const BLOAT_PENALTY: i64 = 150;
/// The value of an option that ends the run.
const DEATH_VALUE: i64 = -3_000;
/// The most actions a measurement rollout plays.
const ROLLOUT_ACTIONS: usize = 4_000;

/// Measures, on seed `seed` (a **train** seed), every event and ancient option the bot meets.
///
/// The option is applied on a copy of the run and the bot plays on to the next map screen; the
/// sample is the change in [`run::run_value`]. This is the offline experience
/// [`values`] looks up; the bot never does it during play.
#[must_use]
pub fn measure_options(
    bot: &Bot,
    character: Character,
    seed: u64,
    ascension: u8,
) -> Vec<(String, i64)> {
    let mut samples = Vec::new();
    let Ok(mut run) = new_run(character, seed, ascension) else {
        return samples;
    };
    let mut actions = 0;
    while terminal(&run, actions).is_none() {
        let keys: Vec<(RunAction, String)> = match run.pending_decision() {
            RunDecision::Event(room) => room
                .options
                .iter()
                .enumerate()
                .map(|(i, o)| (RunAction::EventOption(i), o.text_key.clone()))
                .collect(),
            RunDecision::Ancient(room) => room
                .options
                .iter()
                .enumerate()
                .filter_map(|(i, o)| {
                    o.relic_id()
                        .map(|id| (RunAction::AncientOption(i), values::ancient_key(id)))
                })
                .collect(),
            _ => Vec::new(),
        };
        let before = run::run_value(run.state());
        let deck = run.state().deck.len();
        for (action, key) in keys {
            let mut copy = run.clone();
            if copy.apply(action).is_err() {
                continue;
            }
            let mut n = 0;
            while n < ROLLOUT_ACTIONS
                && terminal(&copy, 0).is_none()
                && !matches!(copy.pending_decision(), RunDecision::Map(_))
            {
                if step(&mut copy, bot).is_none() {
                    break;
                }
                n += 1;
            }
            let value = if matches!(copy.pending_decision(), RunDecision::GameOver) {
                DEATH_VALUE
            } else {
                let grown = copy.state().deck.len().saturating_sub(deck + 2);
                run::run_value(copy.state())
                    - before
                    - BLOAT_PENALTY * i64::try_from(grown).unwrap_or(0)
            };
            samples.push((key, value));
        }
        if step(&mut run, bot).is_none() {
            break;
        }
        actions += 1;
    }
    samples
}

/// The measured samples as the source of [`table::OPTION_VALUES`].
#[must_use]
pub fn table_source(samples: &[(String, i64)]) -> String {
    let mut by_key: BTreeMap<&str, (i64, u32)> = BTreeMap::new();
    for (key, value) in samples {
        let e = by_key.entry(key.as_str()).or_default();
        e.0 += value;
        e.1 += 1;
    }
    let mut out = String::from(
        "//! Measured event and ancient option values (see [`super::values`]), sorted by key:\n\
         //! `(key, mean run-value change, samples)`. Generated on the train seeds by\n\
         //! `sts2-diff bot --measure-options`; regenerate it, do not edit it by hand.\n\n\
         pub const OPTION_VALUES: &[(&str, i32, u32)] = &[\n",
    );
    for (key, (sum, n)) in by_key {
        let mean = sum / i64::from(n);
        let _ = writeln!(out, "    ({key:?}, {mean}, {n}),");
    }
    out.push_str("];\n");
    out
}

/// A fight start met on a run: the position and the context the run shows.
#[derive(Clone, Debug)]
pub struct FightStart {
    pub seed: u64,
    pub combat: sts2_core::Combat,
    pub context: FightContext,
}

/// The fight starts `bot` meets on seed `seed` (a **train** seed): every elite and boss fight
/// and every `common_every`-th common one.
#[must_use]
pub fn fight_starts(
    bot: &Bot,
    character: Character,
    seed: u64,
    ascension: u8,
    common_every: usize,
) -> Vec<FightStart> {
    let mut out = Vec::new();
    let Ok(mut run) = new_run(character, seed, ascension) else {
        return out;
    };
    let mut actions = 0;
    let mut in_fight = false;
    let mut commons = 0;
    while terminal(&run, actions).is_none() {
        let fighting = run.combat().is_some();
        if fighting && !in_fight {
            let context = context_of(run.state());
            let take = context.big || {
                commons += 1;
                commons % common_every.max(1) == 0
            };
            if let (true, Some(combat)) = (take, run.combat()) {
                out.push(FightStart {
                    seed,
                    combat: combat.clone(),
                    context,
                });
            }
        }
        in_fight = fighting;
        if step(&mut run, bot).is_none() {
            break;
        }
        actions += 1;
    }
    out
}

/// The start record of every fight `bot` meets on seed `seed` (a **train** seed).
///
/// Each comes with the context the run shows; the count is of the fights whose record does
/// not rebuild the run's combat to the same observation (those are left out).
#[must_use]
pub fn fight_records(
    bot: &Bot,
    character: Character,
    seed: u64,
    ascension: u8,
) -> (Vec<(sts2_rl::StartRecord, FightContext)>, usize) {
    let mut out = Vec::new();
    let mut mismatched = 0;
    let Ok(run) = new_run(character, seed, ascension) else {
        return (out, 0);
    };
    let mut run = run.with_combat_start_streams();
    let mut actions = 0;
    let mut in_fight = false;
    while terminal(&run, actions).is_none() {
        let fighting = run.combat().is_some();
        if fighting
            && !in_fight
            && let Some(record) = sts2_rl::StartRecord::from_run(&run, seed_name(seed))
            && let Some(combat) = run.combat()
        {
            if record.combat().is_some_and(|c| same_view(&c, combat)) {
                out.push((record, context_of(run.state())));
            } else {
                mismatched += 1;
            }
        }
        in_fight = fighting;
        if step(&mut run, bot).is_none() {
            break;
        }
        actions += 1;
    }
    (out, mismatched)
}

/// A position the bot met at the start of one of its turns, with how the fight it was in
/// ended under the bot's own play: the expert-iteration target of the value net (RLB12).
#[derive(Clone, Debug)]
pub struct ValueSample {
    /// [`value::observation`]: what the player sees, the dynamic part then the static part.
    pub obs: Vec<u8>,
    pub context: FightContext,
    pub won: bool,
    /// The HP the fight ended on and the player's max HP then (0 for a loss).
    pub hp: i32,
    pub max_hp: i32,
}

/// The start record of every fight `bot` meets on seed `seed` (a **train** seed), as
/// [`fight_records`], and the observation at the start of each of its turns in them with the
/// fight's outcome ([`ValueSample`]).
#[must_use]
pub fn value_records(
    bot: &Bot,
    character: Character,
    seed: u64,
    ascension: u8,
) -> (Vec<(sts2_rl::StartRecord, FightContext)>, Vec<ValueSample>) {
    let mut starts = Vec::new();
    let mut samples = Vec::new();
    let Ok(run) = new_run(character, seed, ascension) else {
        return (starts, samples);
    };
    let mut run = run.with_combat_start_streams();
    let mut actions = 0;
    let mut in_fight = false;
    // This fight's samples (from `fight_from`), its context, the turn last recorded and the
    // HP it ended on.
    let mut fight_from = 0;
    let mut context = context_of(run.state());
    let mut turn = 0;
    let mut ended: Option<(i32, i32)> = None;
    let close = |samples: &mut Vec<ValueSample>, from: usize, ended: Option<(i32, i32)>| {
        let (hp, max_hp) = ended.filter(|e| e.0 > 0).unwrap_or((0, 1));
        for sample in &mut samples[from..] {
            sample.won = hp > 0;
            sample.hp = hp;
            sample.max_hp = max_hp;
        }
    };
    while terminal(&run, actions).is_none() {
        let fighting = run.combat().is_some();
        if fighting && !in_fight {
            context = context_of(run.state());
            fight_from = samples.len();
            turn = 0;
            ended = None;
            if let Some(record) = sts2_rl::StartRecord::from_run(&run, seed_name(seed))
                && let Some(combat) = run.combat()
                && record.combat().is_some_and(|c| same_view(&c, combat))
            {
                starts.push((record, context));
            }
        }
        if !fighting && in_fight {
            let s = run.state();
            close(&mut samples, fight_from, ended.or(Some((s.hp, s.max_hp))));
        }
        in_fight = fighting;
        let mut legal = run.legal_actions();
        let action = loop {
            if legal.is_empty() {
                break None;
            }
            let action = bot.choose(run.state(), &run.pending_decision(), &legal);
            // A combat action: the observation at a turn's start, and the HP an ending leaves.
            if let (RunAction::Combat(a), Some(combat)) = (&action, run.combat()) {
                let s = combat.state();
                if s.player.turn_number != turn && combat.pending_decision().is_none() {
                    turn = s.player.turn_number;
                    samples.push(ValueSample {
                        obs: value::observation(combat),
                        context,
                        won: false,
                        hp: 0,
                        max_hp: 1,
                    });
                }
                let mut next = combat.clone();
                if next.apply(a.clone()).is_ok() && next.state().result.is_some() {
                    let s = next.state();
                    let me = s.creature(s.player.creature);
                    ended = Some((me.hp, me.max_hp));
                }
            }
            if run.apply(action.clone()).is_ok() {
                break Some(action);
            }
            legal.retain(|a| *a != action);
        };
        if action.is_none() {
            break;
        }
        actions += 1;
    }
    // A run that ends in a fight (a death) closes it as lost.
    if in_fight {
        close(&mut samples, fight_from, None);
    }
    (starts, samples)
}

/// Whether two fights show the player the same observation.
fn same_view(a: &sts2_core::Combat, b: &sts2_core::Combat) -> bool {
    use sts2_rl::obs::{OBS_BYTES, Observer, STATIC_BYTES, observe_static};
    let view = |c: &sts2_core::Combat| {
        let mut out = vec![0; OBS_BYTES + STATIC_BYTES];
        let (dynamic, statics) = out.split_at_mut(OBS_BYTES);
        Observer::new(true).observe(c, dynamic);
        observe_static(c, statics);
        out
    };
    view(a) == view(b)
}

/// One fight played by `bot`'s controller: won, and the HP it ended on. With `record`, each
/// move it makes that the playout policy could make is kept as a training sample.
#[must_use]
pub fn play_fight(
    start: &FightStart,
    bot: &Bot,
    record: Option<&mut Vec<rollout::Sample>>,
) -> (bool, i32) {
    play_fight_with(start, bot, record, None)
}

/// [`play_fight`], keeping in `positions` the fight at the start of each of the player's turns
/// (RLB15: the search's own positions, to train the playout policy from).
#[must_use]
pub fn play_fight_with(
    start: &FightStart,
    bot: &Bot,
    mut record: Option<&mut Vec<rollout::Sample>>,
    mut positions: Option<&mut Vec<sts2_core::Combat>>,
) -> (bool, i32) {
    let mut combat = start.combat.clone();
    let mut legal = Vec::new();
    let mut scratch = rollout::Scratch::default();
    let mut turn = 0;
    for _ in 0..3_000 {
        if combat.state().result.is_some() || combat.state().player.turn_number > MAX_COMBAT_TURNS {
            break;
        }
        combat.legal_actions_into(&mut legal);
        if legal.is_empty() {
            break;
        }
        if let Some(positions) = positions.as_deref_mut()
            && combat.pending_decision().is_none()
            && combat.state().player.turn_number != turn
        {
            turn = combat.state().player.turn_number;
            positions.push(combat.clone());
        }
        let action = bot.fight_action(&combat, &legal, start.context);
        if let Some(samples) = record.as_deref_mut()
            && combat.pending_decision().is_none()
            && let Some(sample) = rollout::sample_of(&combat, &legal, &action, &mut scratch)
        {
            samples.push(sample);
        }
        if combat.apply(action).is_err() {
            break;
        }
    }
    let s = combat.state();
    let hp = s.creature(s.player.creature).hp;
    (
        s.result == Some(sts2_core::combat::CombatResult::Won) && hp > 0,
        hp.max(0),
    )
}
