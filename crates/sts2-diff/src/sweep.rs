//! `sts2-diff sweep`: tallies why whole seeded runs on the engine alone (no referee) cannot go on.
//!
//! The reasons: a fight stopped on unported content, a stub room, an
//! action the policy picked from `legal_actions` that `apply` refused (an unported relic, potion,
//! card or encounter, or a real engine error), a screen where every legal action was refused, a
//! run that never ends, and panics.
//!
//! A run's seed, character, ascension, Act 1 and policy fully determine it, so the table's
//! "first repro" column is a command that replays the first run to hit a reason.
//!
//! The sweep fails (`sts2-diff sweep` exits 1) on an anomaly: a panic, an engine error or a run
//! that does not end within the action budget. A run that takes longer than `--max-seconds` is
//! reported as `SLOW` and is not one: wall time depends on the machine's load, so it would not
//! reproduce.
//!
//! `--emit-starts FILE` also writes the RL start corpus (`docs/specs/rl-2x-plan.md`, section
//! 7.5): one [`StartRecord`] per Act 1 combat the runs enter, as JSON lines in job order.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use sts2_core::Character;
use sts2_core::combat::Action;
use sts2_core::content::cards::TargetType;
use sts2_core::run::{Act1, Room, Run, RunAction, RunDecision, RunError, UnlockState};

use sts2_rl::StartRecord;

use crate::args::Args;
use crate::greedy::Greedy;
use crate::policy::Policy;
use crate::{Error, Result};

/// The `--flag value` flags `sweep` takes.
pub const VALUE_FLAGS: [&str; 11] = [
    "--runs",
    "--character",
    "--asc",
    "--asc-range",
    "--policy",
    "--act1",
    "--threads",
    "--max-actions",
    "--max-seconds",
    "--seed",
    "--emit-starts",
];
/// The presence flags `sweep` takes.
pub const PRESENCE_FLAGS: [&str; 3] = ["--all", "--strong", "--capacities"];

/// Actions a run takes before it counts as never ending.
const DEFAULT_MAX_ACTIONS: usize = 60_000;
/// Seconds a run may take before it stops as slow (not an anomaly).
const DEFAULT_MAX_SECONDS: u64 = 60;
/// Card plays in one turn before the random policy ends it (zero-cost loops).
const MAX_PLAYS_PER_TURN: usize = 40;
/// HP and gold given at the first map by `--strong` (as the TUI's `whole_game` test does).
const STRONG_HP: i32 = 9_999;
const STRONG_GOLD: i32 = 5_000;
/// Stack of a worker thread: the engine recurses deeply in long fights.
const STACK_BYTES: usize = 64 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyKind {
    Random,
    Greedy,
}

impl PolicyKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Greedy => "greedy",
        }
    }
}

/// What a finding is, in the order the table lists kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// A fight stopped on unported content.
    Stopped,
    /// Every legal action of a screen was refused.
    Stuck,
    /// The engine panicked.
    Panic,
    /// The run did not end within the action budget.
    Endless,
    /// The run took longer than `--max-seconds` and was stopped.
    Slow,
    /// `apply` refused a legal action with an engine error (not "not ported").
    Error,
    /// `apply` refused a legal action for unported content (the policy went on elsewhere).
    Reject,
    /// A stub room (`RunDecision::Unsupported`).
    Room,
}

impl Kind {
    const fn label(self) -> &'static str {
        match self {
            Self::Stopped => "STOPPED",
            Self::Stuck => "STUCK",
            Self::Panic => "PANIC",
            Self::Endless => "NO-END",
            Self::Slow => "SLOW",
            Self::Error => "ERROR",
            Self::Reject => "REJECT",
            Self::Room => "ROOM",
        }
    }
}

#[derive(Clone, Debug)]
struct Finding {
    kind: Kind,
    reason: String,
    /// The room and floor it happened in.
    place: String,
    /// The act (1-based) it happened in.
    act: usize,
    /// Whether it ended the run.
    fatal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Victory,
    Death,
    Stopped,
    Stuck,
    Endless,
    Slow,
    Panic,
}

#[derive(Clone, Debug)]
struct Job {
    character: Character,
    ascension: u8,
    act1: Act1,
    seed: String,
}

#[derive(Debug)]
struct Config {
    runs: usize,
    characters: Vec<Character>,
    ascensions: Vec<u8>,
    act1s: Vec<Act1>,
    policy: PolicyKind,
    strong: bool,
    /// Record the capacity survey's high-water marks and print them after the table.
    capacities: bool,
    threads: usize,
    max_actions: usize,
    /// Wall-clock seconds one run may take (the greedy policy plans slowly).
    max_seconds: u64,
    seed: Option<String>,
    /// Where to write the start corpus.
    emit_starts: Option<String>,
}

#[derive(Debug)]
struct Report {
    job: Job,
    end: End,
    findings: Vec<Finding>,
    /// The act (1-based) the run was in when it ended or last moved.
    act: usize,
    actions: usize,
    /// The start records of the run's Act 1 combats (`--emit-starts`), as JSON lines.
    starts: Vec<String>,
}

/// What a run has recorded so far; kept outside `play` so a panic leaves it intact.
#[derive(Default, Debug)]
struct Progress {
    findings: Vec<Finding>,
    place: String,
    act: usize,
    actions: usize,
    last_action: String,
    starts: Vec<String>,
}

impl Progress {
    fn add(&mut self, kind: Kind, reason: String, fatal: bool) {
        if let Some(f) = self
            .findings
            .iter_mut()
            .find(|f| f.kind == kind && f.reason == reason)
        {
            f.fatal |= fatal;
            return;
        }
        let place = self.place.clone();
        self.findings.push(Finding {
            kind,
            reason,
            place,
            act: self.act,
            fatal,
        });
    }
}

thread_local! {
    /// The last panic of this thread: message and location (the hook keeps it quiet).
    static LAST_PANIC: RefCell<String> = const { RefCell::new(String::new()) };
}

fn parse_config(args: &Args) -> Result<Config> {
    let flag = |name: &str| args.get_opt(name);
    let characters = match (flag("--character"), args.has("--all")) {
        (Some(_), true) => return Err("--character and --all exclude each other".into()),
        (Some(name), false) => {
            vec![Character::from_name(name).ok_or_else(|| format!("unknown character {name}"))?]
        }
        (None, _) => Character::ALL.to_vec(),
    };
    let ascensions: Vec<u8> = if let Some(range) = flag("--asc-range") {
        let (from, to) = range
            .split_once("..")
            .ok_or("--asc-range wants A..B (inclusive)")?;
        (from.parse::<u8>()?..=to.parse::<u8>()?).collect()
    } else {
        vec![args.parse_or("--asc", 0)?]
    };
    if flag("--asc").is_some() && flag("--asc-range").is_some() {
        return Err("--asc and --asc-range exclude each other".into());
    }
    let act1s = match flag("--act1").unwrap_or("both") {
        "both" => vec![Act1::Overgrowth, Act1::Underdocks],
        "overgrowth" => vec![Act1::Overgrowth],
        "underdocks" => vec![Act1::Underdocks],
        other => return Err(format!("unknown --act1 {other}").into()),
    };
    let policy = match flag("--policy").unwrap_or("random") {
        "random" => PolicyKind::Random,
        "greedy" => PolicyKind::Greedy,
        other => return Err(format!("unknown --policy {other}").into()),
    };
    if ascensions.is_empty() {
        return Err("empty --asc-range".into());
    }
    Ok(Config {
        runs: args.parse_or("--runs", 100)?,
        characters,
        ascensions,
        act1s,
        policy,
        strong: args.has("--strong"),
        capacities: args.has("--capacities"),
        threads: args.parse_or("--threads", 8)?,
        max_actions: args.parse_or("--max-actions", DEFAULT_MAX_ACTIONS)?,
        max_seconds: args.parse_or("--max-seconds", DEFAULT_MAX_SECONDS)?,
        seed: flag("--seed").map(str::to_owned),
        emit_starts: flag("--emit-starts").map(str::to_owned),
    })
}

/// The jobs: per character, `runs` runs cycling through the ascensions, then the Act 1s.
fn jobs(config: &Config) -> Vec<Job> {
    let mut jobs = Vec::new();
    for &character in &config.characters {
        if let Some(seed) = &config.seed {
            for &ascension in &config.ascensions {
                for &act1 in &config.act1s {
                    jobs.push(Job {
                        character,
                        ascension,
                        act1,
                        seed: seed.clone(),
                    });
                }
            }
            continue;
        }
        let n = config.ascensions.len();
        for i in 0..config.runs {
            jobs.push(Job {
                character,
                ascension: config.ascensions[i % n],
                act1: config.act1s[(i / n) % config.act1s.len()],
                seed: format!("SW{i}"),
            });
        }
    }
    jobs
}

fn policy_seed(job: &Job) -> u64 {
    let text = format!("{}/{}/{}", job.seed, job.character.name(), job.ascension);
    text.bytes().fold(0xCBF2_9CE4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
    })
}

/// The name of an action's variant.
fn action_kind(action: &RunAction) -> String {
    let text = format!("{action:?}");
    text.split(['(', ' ', '{']).next().unwrap_or("").to_owned()
}

fn decision_kind(run: &Run) -> &'static str {
    match run.pending_decision() {
        RunDecision::Ancient(_) => "ancient",
        RunDecision::Event(_) => "event",
        RunDecision::Map(_) => "map",
        RunDecision::Combat(_) => "combat",
        RunDecision::Rewards(_) => "rewards",
        RunDecision::CardSelect(_) => "card select",
        RunDecision::Bundles(_) => "bundles",
        RunDecision::RestSite(_) => "rest site",
        RunDecision::Treasure(_) => "treasure",
        RunDecision::Shop(_) => "shop",
        RunDecision::Unsupported(_) => "stub room",
        RunDecision::Stopped(_) => "stopped",
        RunDecision::GameOver => "game over",
        RunDecision::Victory => "victory",
    }
}

fn describe_room(run: &Run) -> String {
    format!("{}, floor {}", room_name(run), run.state().act_floor)
}

/// The room without its floor.
fn room_name(run: &Run) -> String {
    let s = run.state();
    match &s.room {
        Room::Ancient { id } => format!("ancient {id}"),
        Room::Map => "map".to_owned(),
        Room::Combat {
            room_type,
            encounter,
        } => format!("{room_type:?} fight {encounter}"),
        Room::Event { id } => format!("event {id}"),
        Room::RestSite => "rest site".to_owned(),
        Room::Treasure => "treasure".to_owned(),
        Room::Shop => "shop".to_owned(),
        Room::Unsupported {
            point_type,
            room_type,
        } => format!(
            "stub {point_type:?}/{room_type:?} (last event {})",
            s.visited_event_ids.last().copied().unwrap_or("-")
        ),
    }
}

/// The random policy's candidates for the current screen, best first (the TUI test's order).
fn ranked(run: &Run, policy: &mut Policy, plays: usize) -> Vec<RunAction> {
    let mut legal = run.legal_actions();
    for i in (1..legal.len()).rev() {
        legal.swap(i, policy.pick(i + 1));
    }
    let rank = |a: &RunAction| -> u8 {
        match (run.pending_decision(), a) {
            (_, RunAction::UsePotion(_) | RunAction::DiscardPotion(_)) => 9,
            (RunDecision::Combat(c), RunAction::Combat(act)) => match act {
                Action::Resolve(_) => 0,
                // Attacks first: the bosses with a clock fall before it runs out.
                Action::PlayCard { card, .. } if plays < MAX_PLAYS_PER_TURN => {
                    1 + u8::from(c.state().target_type(*card) != TargetType::AnyEnemy)
                }
                Action::UsePotion { .. } => 4,
                Action::EndTurn => 5,
                Action::PlayCard { .. } => 8,
            },
            (RunDecision::Rewards(_), RunAction::ClaimReward(_) | RunAction::PickCard { .. })
            | (RunDecision::Treasure(_), RunAction::TakeRelic)
            | (RunDecision::RestSite(_), RunAction::Proceed) => 0,
            (RunDecision::Rewards(_), RunAction::SkipReward(_)) => 3,
            (RunDecision::Rewards(_), RunAction::Proceed) => 4,
            (RunDecision::Shop(_), RunAction::Proceed) => 6,
            (RunDecision::Shop(_), _) | (RunDecision::CardSelect(_), RunAction::ConfirmSelect) => 1,
            (RunDecision::CardSelect(_), RunAction::CancelSelect) => 7,
            _ => 2,
        }
    };
    legal.sort_by_key(rank);
    if let RunDecision::Combat(c) = run.pending_decision()
        && c.status() == sts2_core::Status::PlayerTurn
        && plays >= MAX_PLAYS_PER_TURN
    {
        legal.retain(|a| *a == RunAction::Combat(Action::EndTurn));
    }
    legal
}

/// The last card the player played and the last monster move before a fight stopped.
fn stop_hint(run: &Run) -> String {
    let Some(combat) = run.combat() else {
        return String::new();
    };
    let events = combat.events();
    let card = events.iter().rev().find_map(|e| match e {
        sts2_core::combat::Event::CardPlayed { card, .. } => {
            Some(format!("{:?}", combat.state().card(*card).kind))
        }
        _ => None,
    });
    let monster = events.iter().rev().find_map(|e| match e {
        sts2_core::combat::Event::MonsterMove { name, .. } => Some(*name),
        _ => None,
    });
    format!(
        "last card {}, last monster move {}",
        card.as_deref().unwrap_or("-"),
        monster.unwrap_or("-")
    )
}

/// Whether a refused action is content that is not ported or an engine error.
const fn refusal_kind(error: &RunError) -> Kind {
    match error {
        RunError::UnsupportedEncounter(_)
        | RunError::UnsupportedAncientOption(_)
        | RunError::UnsupportedRelic(_)
        | RunError::UnsupportedCard(_)
        | RunError::UnsupportedPotion(_) => Kind::Reject,
        _ => Kind::Error,
    }
}

/// The greedy policy's pick among the legal actions not yet refused.
fn greedy_choice(run: &Run, planner: &mut Greedy, failed: &[RunAction]) -> Option<RunAction> {
    let modeled: Vec<RunAction> = run
        .legal_actions()
        .into_iter()
        .filter(|a| !failed.contains(a))
        .collect();
    if modeled.is_empty() {
        return None;
    }
    if failed.is_empty()
        && let Some(answer) = run.combat().and_then(Greedy::answer)
    {
        return Some(RunAction::Combat(answer));
    }
    Some(planner.choose(run, &modeled))
}

/// Records what the pending decision says about the run; `Some` when the run is over.
/// Gives the `--strong` loadout at the first map.
fn note_decision(run: &mut Run, progress: &mut Progress, loaded: &mut bool) -> Option<End> {
    match run.pending_decision() {
        RunDecision::Victory => return Some(End::Victory),
        RunDecision::GameOver => return Some(End::Death),
        RunDecision::Stopped(unsupported) => {
            progress.place = format!("{}; {}", progress.place, stop_hint(run));
            progress.add(Kind::Stopped, unsupported.to_string(), true);
            return Some(End::Stopped);
        }
        RunDecision::Unsupported(point) => {
            progress.add(Kind::Room, format!("{point:?}: {}", room_name(run)), false);
        }
        RunDecision::Map(_) if !*loaded => {
            *loaded = true;
            if let Ok(state) = run.loadout_state() {
                state.hp = STRONG_HP;
                state.max_hp = STRONG_HP;
                state.gold = STRONG_GOLD;
            }
        }
        _ => {}
    }
    None
}

/// Records the start of the Act 1 combat `run` just entered, if it did.
fn note_start(job: &Job, run: &Run, progress: &mut Progress) {
    if run.state().act_index != 0 {
        return;
    }
    let seed = format!(
        "{}/{}/A{}/{}/F{}",
        job.seed,
        job.character.name(),
        job.ascension,
        job.act1.key(),
        run.state().total_floor()
    );
    if let Some(record) = StartRecord::from_run(run, seed) {
        progress.starts.push(record.to_json());
    }
}

/// Plays one run, recording into `progress`.
fn play(job: &Job, config: &Config, progress: &mut Progress) -> End {
    let mut run = match Run::new(
        job.character,
        &job.seed,
        job.ascension,
        UnlockState::all(),
        job.act1,
    ) {
        Ok(run) if config.emit_starts.is_some() => run.with_events().with_combat_start_streams(),
        Ok(run) => run.with_events(),
        Err(e) => {
            progress.add(Kind::Stuck, format!("Run::new: {e}"), true);
            return End::Stuck;
        }
    };
    let mut policy = Policy::new(policy_seed(job));
    let mut planner = Greedy::default();
    let mut loaded = !config.strong;
    let mut plays = 0;
    let mut turn_marker = None;
    let mut room_seen: Option<Room> = None;
    let started = Instant::now();
    for _ in 0..config.max_actions {
        progress.actions += 1;
        if started.elapsed().as_secs() >= config.max_seconds {
            progress.add(
                Kind::Slow,
                format!("stopped after {}s", config.max_seconds),
                true,
            );
            return End::Slow;
        }
        progress.act = run.state().act_index + 1;
        if room_seen.as_ref() != Some(&run.state().room) {
            room_seen = Some(run.state().room.clone());
            progress.place = describe_room(&run);
        }
        if let Some(end) = note_decision(&mut run, progress, &mut loaded) {
            return end;
        }
        let marker = run.combat().map(|c| (c.state().round, c.status()));
        if marker != turn_marker {
            plays = 0;
            turn_marker = marker;
        }
        let list = if config.policy == PolicyKind::Random {
            ranked(&run, &mut policy, plays)
        } else {
            Vec::new()
        };
        let mut failed: Vec<RunAction> = Vec::new();
        let mut reasons = BTreeSet::new();
        let accepted = loop {
            let candidate = match config.policy {
                PolicyKind::Random => list.iter().find(|a| !failed.contains(a)).cloned(),
                PolicyKind::Greedy => greedy_choice(&run, &mut planner, &failed),
            };
            let Some(action) = candidate else {
                break false;
            };
            progress.last_action = format!("{action:?}");
            let was_in_combat = run.combat().is_some();
            match run.apply(action.clone()) {
                Ok(()) => {
                    if config.emit_starts.is_some() && !was_in_combat {
                        note_start(job, &run, progress);
                    }
                    if let Some(combat) = run.combat() {
                        crate::capacity::observe(combat, || {
                            format!("{} act {}", job.seed, progress.act)
                        });
                    }
                    if matches!(action, RunAction::Combat(Action::PlayCard { .. })) {
                        plays += 1;
                    }
                    break true;
                }
                Err(e) => {
                    let reason = format!("{e} on {}", action_kind(&action));
                    progress.add(refusal_kind(&e), reason.clone(), false);
                    reasons.insert(reason);
                    failed.push(action);
                }
            }
        };
        if !accepted {
            let joined = reasons.into_iter().collect::<Vec<_>>().join("; ");
            progress.add(
                Kind::Stuck,
                format!("{}: {joined}", decision_kind(&run)),
                true,
            );
            return End::Stuck;
        }
    }
    progress.add(
        Kind::Endless,
        format!("no end within {} actions", config.max_actions),
        true,
    );
    End::Endless
}

/// Plays `job`, turning a panic into a finding.
fn play_caught(job: &Job, config: &Config) -> Report {
    let mut progress = Progress::default();
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| play(job, config, &mut progress)));
    let end = outcome.unwrap_or_else(|_| {
        let message = LAST_PANIC.with(RefCell::take);
        let last = progress.last_action.clone();
        progress.add(Kind::Panic, message, true);
        // Keep the last action in the place so the panic can be located.
        if let Some(f) = progress.findings.last_mut() {
            f.place = format!("{} after {last}", f.place);
        }
        End::Panic
    });
    Report {
        job: job.clone(),
        end,
        findings: progress.findings,
        act: progress.act,
        actions: progress.actions,
        starts: progress.starts,
    }
}

fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let first = message.lines().next().unwrap_or_default().to_owned();
        let at = info
            .location()
            .map_or_else(String::new, |l| format!(" ({}:{})", l.file(), l.line()));
        LAST_PANIC.with(|p| *p.borrow_mut() = format!("{first}{at}"));
    }));
}

/// The command that replays `job` alone.
fn repro(job: &Job, config: &Config) -> String {
    format!(
        "cargo xtask sweep --seed {} --character {} --asc {} --act1 {} --policy {}{}",
        job.seed,
        job.character.name(),
        job.ascension,
        job.act1.key(),
        config.policy.name(),
        if config.strong { " --strong" } else { "" },
    )
}

/// One row of the reasons table.
#[derive(Default, Debug)]
struct Row {
    runs: usize,
    fatal: usize,
    characters: BTreeSet<&'static str>,
    acts: BTreeSet<usize>,
    place: String,
    repro: String,
}

fn escape(text: &str) -> String {
    text.replace('|', "\\|")
}

/// Kinds that are engine bugs, not unported content: any one of them fails the sweep.
const ANOMALIES: [Kind; 3] = [Kind::Panic, Kind::Error, Kind::Endless];

/// Plays the sweep described by `args` and returns the report lines (markdown), and whether no
/// run found an anomaly ([`ANOMALIES`]).
///
/// # Errors
/// On bad arguments.
///
/// # Panics
/// If a worker thread panics outside a run.
pub fn run(args: &Args) -> Result<(Vec<String>, bool)> {
    let config = parse_config(args)?;
    if config.capacities {
        crate::capacity::enable();
    }
    let jobs = jobs(&config);
    install_panic_hook();
    let started = Instant::now();
    let next = AtomicUsize::new(0);
    let reports: Mutex<Vec<(usize, Report)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::new();
        for _ in 0..config.threads.max(1) {
            let worker = std::thread::Builder::new()
                .stack_size(STACK_BYTES)
                .spawn_scoped(scope, || {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(i) else { break };
                        let report = play_caught(job, &config);
                        reports
                            .lock()
                            .expect("no worker panics while holding the lock")
                            .push((i, report));
                    }
                    crate::capacity::flush_thread();
                })?;
            handles.push(worker);
        }
        for handle in handles {
            handle.join().map_err(|_| Error::from("worker panicked"))?;
        }
        Ok(())
    })?;
    let mut reports = reports.into_inner().map_err(|_| "poisoned")?;
    reports.sort_by_key(|(i, _)| *i);
    let clean = !reports
        .iter()
        .flat_map(|(_, r)| &r.findings)
        .any(|f| ANOMALIES.contains(&f.kind));
    let mut lines = render(&config, &reports, started.elapsed().as_secs());
    if let Some(path) = &config.emit_starts {
        let starts: Vec<&str> = reports
            .iter()
            .flat_map(|(_, r)| &r.starts)
            .map(String::as_str)
            .collect();
        let mut text = starts.join("\n");
        text.push('\n');
        std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))?;
        lines.push(String::new());
        lines.push(format!("Wrote {} starts to {path}.", starts.len()));
    }
    if config.capacities {
        lines.push(String::new());
        lines.push("Capacity survey (high-water marks between steps):".to_owned());
        lines.push(String::new());
        lines.extend(
            crate::capacity::take()
                .to_string()
                .lines()
                .map(str::to_owned),
        );
    }
    if !clean {
        lines.push(String::new());
        lines.push("sweep: FAIL (a PANIC, ERROR or NO-END row above)".to_owned());
    }
    Ok((lines, clean))
}

fn render(config: &Config, reports: &[(usize, Report)], seconds: u64) -> Vec<String> {
    let mut rows: BTreeMap<(Kind, String), Row> = BTreeMap::new();
    for (_, report) in reports {
        for f in &report.findings {
            let row = rows.entry((f.kind, f.reason.clone())).or_default();
            if row.runs == 0 {
                row.place.clone_from(&f.place);
                row.repro = repro(&report.job, config);
            }
            row.runs += 1;
            row.fatal += usize::from(f.fatal);
            row.characters.insert(report.job.character.name());
            row.acts.insert(f.act);
        }
    }
    let mut ordered: Vec<(&(Kind, String), &Row)> = rows.iter().collect();
    ordered.sort_by(|a, b| (b.1.fatal, b.1.runs, a.0.0).cmp(&(a.1.fatal, a.1.runs, b.0.0)));
    let count = |end: End| reports.iter().filter(|(_, r)| r.end == end).count();
    let mut lines = vec![
        "| kind | reason | runs | ended run | characters | acts | first seen at | first repro |"
            .to_owned(),
        "|---|---|---:|---:|---|---|---|---|".to_owned(),
    ];
    for ((kind, reason), row) in ordered {
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | `{}` |",
            kind.label(),
            escape(reason),
            row.runs,
            row.fatal,
            row.characters
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(", "),
            row.acts
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            escape(&row.place),
            row.repro,
        ));
    }
    let stopped = count(End::Stopped) + count(End::Stuck);
    let mut totals = String::new();
    let _ = write!(
        totals,
        "totals: runs {}, victories {}, deaths {}, stopped {} (fight stopped {}, stuck {}), no-end {}, slow {}, panics {}; {} runs/s over {}s ({} policy{}, {} actions)",
        reports.len(),
        count(End::Victory),
        count(End::Death),
        stopped,
        count(End::Stopped),
        count(End::Stuck),
        count(End::Endless),
        count(End::Slow),
        count(End::Panic),
        reports.len() as u64 / seconds.max(1),
        seconds,
        config.policy.name(),
        if config.strong { ", strong" } else { "" },
        reports.iter().map(|(_, r)| r.actions).sum::<usize>(),
    );
    lines.push(String::new());
    lines.push(totals);
    lines.push(String::new());
    lines.push("| character | runs | victories | deaths | stopped/stuck | no-end | panics | furthest act |".to_owned());
    lines.push("|---|---:|---:|---:|---:|---:|---:|---:|".to_owned());
    for &character in &config.characters {
        let mine: Vec<&Report> = reports
            .iter()
            .map(|(_, r)| r)
            .filter(|r| r.job.character == character)
            .collect();
        let n = |ends: &[End]| mine.iter().filter(|r| ends.contains(&r.end)).count();
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            character.name(),
            mine.len(),
            n(&[End::Victory]),
            n(&[End::Death]),
            n(&[End::Stopped, End::Stuck]),
            n(&[End::Endless]),
            n(&[End::Panic]),
            mine.iter().map(|r| r.act).max().unwrap_or(0),
        ));
    }
    lines
}
