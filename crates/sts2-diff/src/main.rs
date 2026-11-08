//! `sts2-diff gen|compare|fixture`: the pieces `tools/diff.sh` drives.

use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};
use sts2_core::Character;
use sts2_core::combat::{Action, Encounter};
use sts2_core::content::ancients::AncientKind;
use sts2_core::run::{Act1, RestOptionKind, Run, RunAction, RunDecision};
use sts2_diff::args::Args;
use sts2_diff::compare::{first_divergence, first_run_divergence};
use sts2_diff::fixture;
use sts2_diff::greedy;
use sts2_diff::hits::Hits;
use sts2_diff::ids;
use sts2_diff::map;
use sts2_diff::policy::Policy;
use sts2_diff::run_session::{self, RunSession};
use sts2_diff::session::{
    ASCENSION, Session, character_or_default, encounter_or_default, whole_choice,
};
use sts2_diff::{Error, Result};

const USAGE: &str = "usage:
  sts2-diff gen --seed S --policy-seed P --max-actions N [--character NAME] [--encounter ID] [--ascension N] [--loadout LOADOUT.json] [--hits OUT.json] --actions OUT.json --trace OUT.jsonl
  sts2-diff compare --rust RUST.jsonl --referee REFEREE.jsonl
  sts2-diff gen-batch --prefix P --count N --dir DIR --max-actions N [gen options as above, no --seed/--policy-seed/--hits/--actions/--trace]
  sts2-diff compare-batch --prefix P --count N --dir DIR
  sts2-diff run-gen --seed S --policy-seed P --max-actions N [--policy random|greedy|shopper] [--prefer-relics ID,ID] [--run-loadout FILE] [--act1 overgrowth|underdocks] [--event ID] [--ancient 0|1] [--character NAME] [--ascension N] [--real-game 0|1] [--hits OUT.json] --actions OUT.json --trace OUT.jsonl
  sts2-diff compare-run --rust RUST.jsonl --referee REFEREE.jsonl
  sts2-diff map --seed S [--act 0|1|2] [--ascension N]
  sts2-diff compare-map --seed S --referee REFEREE.jsonl [--act 0|1|2 --ascension N]   (--act: against run.act_maps)
  sts2-diff history-predict [--batch] < INPUT.json   (see tools/replay_history.py; --batch: one input per line)
  sts2-diff fixture --seed S --actions ACTIONS.json --referee REFEREE.jsonl --out FIXTURE.json
  sts2-diff run-fixture --seed S --actions ACTIONS.json --referee REFEREE.jsonl [--steps N] --out FIXTURE.json
  sts2-diff sweep [--runs N (per character)] [--character C | --all] [--asc A | --asc-range A..B (inclusive)] [--policy random|greedy] [--act1 overgrowth|underdocks|both] [--strong] [--capacities] [--threads N] [--max-actions N] [--max-seconds N] [--seed S] [--emit-starts FILE]
  sts2-diff reach (--seed S --character C --ascension N --act1 overgrowth|underdocks [--max-actions N] [--record FILE] | --replay FILE) [--verbose]
  sts2-diff bot --first N --count M [--controller tree|search|ppo] [--sims N] [--big-sims N] [--boss-sims N] [--c X] [--widen-k X] [--widen-alpha X] [--turns N] [--playouts-all] [--weights FILE] [--worlds K] [--big-worlds K] [--boss-worlds K] [--beam B] [--no-playouts] [--net-playouts] [--value FILE|builtin|vn1 [--value-leaves] [--value-hp N] [--value-last-boss-hp N] [--value-mid-boss-hp N] [--value-boss-potion N] [--value-keep K] [--value-in all|big|boss] [--value-potion N] [--value-potion-road N [--value-potion-road-act A]] [--value-prior N] [--rollout FILE|builtin|v2 --value-mix N [--value-mix-ends N] [--value-mix-samples N] [--value-mix-turns N] [--value-mix-in boss|big|all]]] [--character C] [--ascension N] [--jobs N] [--out DIR] [--measure-options] [--run-policy FILE|builtin] [--seeds FILE] [--search-salt N] [--remove-curses] [--deck-probe WORLDS,MARGIN,WEIGHT] [--minions kills|hp|primary]
  sts2-diff run-log --dirs DIR[,DIR] --out OUT.jsonl
  sts2-diff boss-starts --dirs DIR[,DIR] [--jobs N] --out STARTS.jsonl
  sts2-diff deck-probe --dirs DIR[,DIR] [--worlds N] [--rollout FILE|v2] [--jobs N] --out OUT.tsv
  sts2-diff collect-starts --first N --count M [--run-policy FILE|builtin] [--start-value FILE|builtin] [--jobs N] --out FILE.jsonl
  sts2-diff value-collect --first N --count M [--run-policy FILE|builtin] [--start-value FILE|builtin] [--jobs N] --out DIR
  sts2-diff run-collect --first N --count M [--policy FILE [--greedy]] [--rng N] [--worlds K] [--big-worlds K] [--boss-worlds K] [--beam B] [--ascension N] [--jobs N] --out DIR
  sts2-diff capacities DIR  (merges the files STS2_CAPACITIES=DIR left; see crates/sts2-diff/src/capacity.rs)";

/// The flags `gen` and `gen-batch` share.
const GEN_FLAGS: [&str; 5] = [
    "--max-actions",
    "--character",
    "--encounter",
    "--ascension",
    "--loadout",
];

/// The `--flag value` flags and the presence flags each command takes; any other flag is an error
/// ([`Args::parse`]).
#[allow(
    clippy::too_many_lines,
    reason = "one flag list per command; splitting it would only scatter the table"
)]
fn flags(command: &str) -> Option<(Vec<&'static str>, &'static [&'static str])> {
    let with = |extra: &[&'static str]| [&GEN_FLAGS[..], extra].concat();
    Some(match command {
        "gen" => (
            with(&["--seed", "--policy-seed", "--hits", "--actions", "--trace"]),
            &[],
        ),
        "gen-batch" => (with(&["--prefix", "--count", "--dir"]), &[]),
        "compare" | "compare-run" => (vec!["--rust", "--referee"], &[]),
        "compare-batch" => (vec!["--prefix", "--count", "--dir"], &[]),
        "run-gen" => (
            vec![
                "--seed",
                "--policy-seed",
                "--max-actions",
                "--policy",
                "--prefer-relics",
                "--run-loadout",
                "--act1",
                "--event",
                "--ancient",
                "--ascension",
                "--character",
                "--real-game",
                "--hits",
                "--actions",
                "--trace",
            ],
            &[],
        ),
        "map" => (vec!["--seed", "--act", "--ascension"], &[]),
        "compare-map" => (vec!["--seed", "--referee", "--act", "--ascension"], &[]),
        "history-predict" => (vec![], &["--batch"]),
        "oracle-gaps" => (vec![], &[]),
        "fixture" => (vec!["--seed", "--actions", "--referee", "--out"], &[]),
        "run-fixture" => (
            vec!["--seed", "--actions", "--referee", "--steps", "--out"],
            &[],
        ),
        "sweep" => (
            sts2_diff::sweep::VALUE_FLAGS.to_vec(),
            &sts2_diff::sweep::PRESENCE_FLAGS,
        ),
        "run-log" => (vec!["--dirs", "--out"], &[]),
        "boss-starts" => (vec!["--dirs", "--out", "--jobs"], &[]),
        "deck-probe" => (
            vec!["--dirs", "--out", "--worlds", "--rollout", "--jobs"],
            &[],
        ),
        "reach" => (
            vec![
                "--seed",
                "--character",
                "--ascension",
                "--act1",
                "--max-actions",
                "--record",
                "--replay",
            ],
            &["--verbose"],
        ),
        "fights" | "az-collect" | "search-collect" => (
            vec![
                "--first",
                "--count",
                "--every",
                "--only",
                "--start-policy",
                "--start-value",
                "--starts",
                "--variants",
                "--seeds",
                "--trace",
                "--trace-only",
                "--controller",
                "--weights",
                "--worlds",
                "--big-worlds",
                "--boss-worlds",
                "--beam",
                "--lookahead",
                "--beam2",
                "--depth",
                "--lookahead-in",
                "--close",
                "--close-worlds",
                "--sims",
                "--big-sims",
                "--boss-sims",
                "--c",
                "--widen-k",
                "--widen-alpha",
                "--end-k",
                "--az-in",
                "--turn-prior",
                "--az-batch",
                "--turns",
                "--scale-playout",
                "--scale-turn",
                "--prior-temp",
                "--prior-mix",
                "--rollout",
                "--jobs",
                "--value",
                "--value-in",
                "--value-keep",
                "--value-hp",
                "--value-last-boss-hp",
                "--value-mid-boss-hp",
                "--value-boss-potion",
                "--value-prior",
                "--value-potion",
                "--value-potion-road",
                "--value-potion-road-act",
                "--value-mix",
                "--value-mix-ends",
                "--value-mix-samples",
                "--value-mix-turns",
                "--value-mix-in",
                "--out",
                "--reseed",
                "--reseed-from",
                "--minions",
            ],
            &[
                "--value-leaves",
                "--no-playouts",
                "--net-playouts",
                "--playouts-all",
                "--no-prior",
                "--no-hybrid",
                "--plans",
                "--trace-won",
                "--trace-advice",
            ],
        ),
        "rollout-eval" => (
            vec![
                "--first",
                "--count",
                "--every",
                "--only",
                "--start-policy",
                "--start-value",
                "--starts",
                "--seeds",
                "--worlds",
                "--rollout",
                "--jobs",
            ],
            &[],
        ),
        "rollout-train" => (
            vec![
                "--first",
                "--count",
                "--every",
                "--only",
                "--start-policy",
                "--start-value",
                "--starts",
                "--value",
                "--value-hp",
                "--controller",
                "--worlds",
                "--big-worlds",
                "--boss-worlds",
                "--beam",
                "--lookahead",
                "--beam2",
                "--depth",
                "--lookahead-in",
                "--close",
                "--close-worlds",
                "--sims",
                "--big-sims",
                "--boss-sims",
                "--c",
                "--turns",
                "--scale-playout",
                "--scale-turn",
                "--prior-temp",
                "--prior-mix",
                "--rollout",
                "--init",
                "--hidden",
                "--search-positions",
                "--value",
                "--value-hp",
                "--batch-starts",
                "--epochs",
                "--lr",
                "--iters",
                "--per-start",
                "--rl-lr",
                "--max-starts",
                "--chunk",
                "--rl-count",
                "--jobs",
                "--out",
            ],
            &[
                "--no-playouts",
                "--playouts-all",
                "--no-prior",
                "--no-hybrid",
                "--plans",
                "--value-leaves",
                "--encounters",
            ],
        ),
        "bot" => (
            vec![
                "--first",
                "--count",
                "--controller",
                "--weights",
                "--worlds",
                "--big-worlds",
                "--boss-worlds",
                "--beam",
                "--lookahead",
                "--beam2",
                "--depth",
                "--lookahead-in",
                "--close",
                "--close-worlds",
                "--sims",
                "--big-sims",
                "--boss-sims",
                "--c",
                "--widen-k",
                "--widen-alpha",
                "--end-k",
                "--az-in",
                "--turn-prior",
                "--az-batch",
                "--turns",
                "--scale-playout",
                "--scale-turn",
                "--prior-temp",
                "--prior-mix",
                "--rollout",
                "--character",
                "--ascension",
                "--jobs",
                "--value",
                "--value-in",
                "--value-keep",
                "--value-hp",
                "--value-last-boss-hp",
                "--value-mid-boss-hp",
                "--value-boss-potion",
                "--value-prior",
                "--value-potion",
                "--value-potion-road",
                "--value-potion-road-act",
                "--value-mix",
                "--value-mix-ends",
                "--value-mix-samples",
                "--value-mix-turns",
                "--value-mix-in",
                "--out",
                "--run-policy",
                "--seeds",
                "--search-salt",
                "--deck-probe",
                "--minions",
            ],
            &[
                "--value-leaves",
                "--remove-curses",
                "--measure-options",
                "--clairvoyant-fights",
                "--no-playouts",
                "--net-playouts",
                "--playouts-all",
                "--no-prior",
                "--no-hybrid",
                "--plans",
            ],
        ),
        "collect-starts" | "value-collect" => (
            vec![
                "--first",
                "--count",
                "--seeds",
                "--run-policy",
                "--start-value",
                "--jobs",
                "--out",
            ],
            &[],
        ),
        "run-collect" => (
            vec![
                "--first",
                "--seeds",
                "--count",
                "--policy",
                "--rng",
                "--worlds",
                "--big-worlds",
                "--boss-worlds",
                "--beam",
                "--value",
                "--value-in",
                "--value-keep",
                "--value-hp",
                "--value-last-boss-hp",
                "--value-mid-boss-hp",
                "--value-boss-potion",
                "--value-potion",
                "--value-potion-road",
                "--value-potion-road-act",
                "--ascension",
                "--jobs",
                "--out",
            ],
            &["--greedy", "--value-leaves"],
        ),
        _ => return None,
    })
}

fn read_trace(path: &str) -> Result<Vec<Value>> {
    std::fs::read_to_string(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(Error::from))
        .collect()
}

fn write_trace(path: &str, trace: &[Value]) -> Result<()> {
    let mut out = String::new();
    for line in trace {
        out.push_str(&line.to_string());
        out.push('\n');
    }
    std::fs::write(path, out)?;
    Ok(())
}

fn read_json(path: &str) -> Result<Value> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// Whether run policy `name` is greedy, and whether it shops.
fn policy_flags(name: &str) -> Result<(bool, bool)> {
    match name {
        "random" => Ok((false, false)),
        "greedy" => Ok((true, false)),
        "shopper" => Ok((true, true)),
        other => Err(format!("unknown policy {other}\n{USAGE}").into()),
    }
}

/// What `gen` plays: everything but the seed, shared by the seeds of a `gen-batch`.
struct GenConfig {
    character: Character,
    encounter: Encounter,
    ascension: u8,
    loadout: Option<Value>,
    max_actions: usize,
}

impl GenConfig {
    fn from_args(args: &Args) -> Result<Self> {
        let loadout = args.get_opt("--loadout").map(read_json).transpose()?;
        // `--character`, else the loadout's `"character"`, else Necrobinder.
        let character = character_or_default(args.get_opt("--character").or_else(|| {
            loadout
                .as_ref()
                .and_then(|l| l.get("character"))
                .and_then(Value::as_str)
        }))?;
        Ok(Self {
            character,
            encounter: encounter_or_default(args.get_opt("--encounter"))?,
            ascension: args
                .get_opt("--ascension")
                .map_or(Ok(ASCENSION), str::parse)?,
            loadout,
            max_actions: args.get("--max-actions")?.parse()?,
        })
    }

    /// Plays the seeded random policy until the combat ends or `max_actions` are used; writes the
    /// hits (if `hits_path`), the referee's action file and the trace.
    fn play(
        &self,
        seed: &str,
        policy_seed: u64,
        hits_path: Option<&str>,
        actions_path: &str,
        trace_path: &str,
    ) -> Result<()> {
        let mut policy = Policy::new(policy_seed);
        let mut session = Session::new(
            self.character,
            seed,
            self.encounter,
            self.ascension,
            self.loadout.as_ref(),
        )?;
        let mut trace = vec![session.start_line()];
        let mut actions = Vec::new();
        let mut hits = Hits::default();
        hits.set_scope(self.character.name(), None);
        while !session.is_over() && actions.len() < self.max_actions {
            // An action that would stop the combat at unported content (a generated card) is never
            // taken; the fight stops once only such actions are left.
            let mut legal = session.combat().legal_actions();
            let action = loop {
                let action = whole_choice(session.combat(), |n| policy.pick(n))
                    .unwrap_or_else(|| legal[policy.pick(legal.len())].clone());
                if !session.stops_at(&action) {
                    break Some(action);
                }
                legal.retain(|a| *a != action);
                if legal.is_empty() || session.combat().pending_decision().is_some() {
                    break None;
                }
            };
            let Some(action) = action else {
                break;
            };
            let (referee_action, line) = session.apply(&action)?;
            actions.push(referee_action);
            trace.push(line);
            hits.observe_combat(Some(session.combat()));
        }
        hits.observe_combat(Some(session.combat()));
        let mut file = json!({
            "character": self.character.name(),
            "ascension": self.ascension,
            "encounter": ids::encounter(self.encounter),
            "actions": actions,
        });
        if let Some(path) = hits_path {
            hits.observe_trace(&trace);
            if let Some(loadout) = &self.loadout {
                hits.observe_loadout(loadout);
            }
            hits.write(path)?;
        }
        if let Some(mut loadout) = self.loadout.clone() {
            // Passed to the referee as is, minus the character (its own top-level key).
            if let Some(object) = loadout.as_object_mut() {
                object.remove("character");
            }
            file["loadout"] = loadout;
        }
        std::fs::write(actions_path, serde_json::to_string_pretty(&file)?)?;
        write_trace(trace_path, &trace)
    }
}

/// Plays one seed, see [`GenConfig::play`].
fn gen_cmd(args: &Args) -> Result<()> {
    GenConfig::from_args(args)?.play(
        args.get("--seed")?,
        args.get("--policy-seed")?.parse()?,
        args.get_opt("--hits"),
        args.get("--actions")?,
        args.get("--trace")?,
    )
}

/// The seeds `<prefix>0..<prefix><count-1>` of a batch and the file `<dir>/<seed>.<ext>`.
fn batch_seeds(args: &Args) -> Result<impl Iterator<Item = (u64, String)>> {
    let prefix = args.get("--prefix")?.to_owned();
    let count: u64 = args.get("--count")?.parse()?;
    Ok((0..count).map(move |i| (i, format!("{prefix}{i}"))))
}

/// `gen` for the seeds `<prefix>0..<prefix><count-1>` (policy seed = the index) in one process;
/// files `<dir>/<seed>.hits.pending.json`, `.actions.json` and `.rust.jsonl` (what `tools/diff.sh`
/// names them). An older `<seed>.hits.json` is deleted: `compare-batch` writes it again only if the
/// seed matches.
fn gen_batch_cmd(args: &Args) -> Result<()> {
    let config = GenConfig::from_args(args)?;
    let dir = args.get("--dir")?;
    for (index, seed) in batch_seeds(args)? {
        remove_if_present(&format!("{dir}/{seed}.hits.json"))?;
        config.play(
            &seed,
            index,
            Some(&format!("{dir}/{seed}.hits.pending.json")),
            &format!("{dir}/{seed}.actions.json"),
            &format!("{dir}/{seed}.rust.jsonl"),
        )?;
    }
    Ok(())
}

/// Deletes the file at `path` if there is one.
fn remove_if_present(path: &str) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// `compare` for a batch, one line `<seed>: <result>` per seed (the `compare` output); `Ok(false)`
/// if any diverges. A matching seed's `<seed>.hits.pending.json` becomes its `<seed>.hits.json`
/// (what `cargo xtask hits` reads); a diverging seed's is deleted, so only checked runs count as hits.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn compare_batch_cmd(args: &Args) -> Result<bool> {
    let dir = args.get("--dir")?;
    let mut matching = true;
    for (_, seed) in batch_seeds(args)? {
        let rust = read_trace(&format!("{dir}/{seed}.rust.jsonl"))?;
        let referee = read_trace(&format!("{dir}/{seed}.referee.jsonl"))?;
        let pending = format!("{dir}/{seed}.hits.pending.json");
        if let Some(d) = first_divergence(&rust, &referee) {
            println!("{seed}: DIVERGENCE {d}");
            remove_if_present(&pending)?;
            matching = false;
        } else {
            println!("{seed}: ok ({} steps)", rust.len());
            std::fs::rename(&pending, format!("{dir}/{seed}.hits.json"))?;
        }
    }
    Ok(matching)
}

/// Plays a seeded policy over a whole run until it stops ([`run_stop`]: `died`, `victory`, content
/// that is not ported) or uses `max_actions`; prints `stop=<reason> act=<act reached>
/// floor=<total floor reached> events=<n> shops=<n> bought=<cards>,<relics>,<potions>,<removals>
/// boss=<act 1>,<act 2>,<act 3>,<second>`, each act's boss `reached`, `beaten` or `-`; `second` is the last act's
/// ascension 10 second boss (`reached` once it is fought, `beaten` with the victory, `-` without one).
///
/// `--event ID` (or the run loadout's `"event"`) forces that event the first time the run waits
/// on the map after the loadout (the referee's `enter_event`), takes its first-page option number
/// `policy seed % options` among the modeled ones (so consecutive seeds cover every option), and
/// stops with `event_done` once the run is back on the map; with the run loadout's
/// `"continue": true` the run goes on from there instead (a card the event adds, `TinkerTime`'s
/// `MadScience`, is then fought with).
///
/// `--ancient` (or the run loadout's `"ancient": true`, with its `"act_index"` the act's ancient
/// is the first room) takes option number `policy seed % options` among the modeled ones of the
/// first ancient after the loadout and stops with `ancient_done` once the run is back on the map.
///
/// `--policy random` (the default) picks uniformly among the actions the engine models (in
/// combat it ends the turn only when nothing else is legal); `--policy greedy` uses
/// [`greedy::choose`]. Card choices inside combat are random for both. When no modeled action is
/// left the run stops there.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn run_gen_cmd(args: &Args) -> Result<()> {
    let seed = args.get("--seed")?;
    let mut policy = Policy::new(args.get("--policy-seed")?.parse()?);
    let max_actions: usize = args.get("--max-actions")?.parse()?;
    let (greedy, shopper) = policy_flags(args.get_opt("--policy").unwrap_or("random"))?;
    // Relics the random policy buys whenever a shop offers them (to reach rare content).
    let preferred: Vec<&str> = args
        .get_opt("--prefer-relics")
        .map(|ids| ids.split(',').collect())
        .unwrap_or_default();
    let ascension: u8 = args
        .get_opt("--ascension")
        .map_or(Ok(ASCENSION), str::parse)?;
    let character = character_or_default(args.get_opt("--character"))?;
    let real_game = args.switch("--real-game")?;
    // The run loadout is handed over once, the first time the run waits on the map (after Neow).
    let mut run_loadout = args.get_opt("--run-loadout").map(read_json).transpose()?;
    let act1 = run_act1(args, run_loadout.as_ref())?;
    let mut event = ForcedEvent::from_args(args, &mut run_loadout)?;
    let mut session = RunSession::new(character, seed, ascension, real_game, act1)?;
    let mut trace = vec![session.start_line()];
    let mut actions = Vec::new();
    let mut planner = greedy::Greedy::default();
    let mut hits = Hits::default();
    hits.set_scope(character.name(), Some(act1.key()));
    let stop = loop {
        hits.observe_history(&session.run().state().history);
        observe_effects(&session, &mut hits);
        if matches!(session.run().pending_decision(), RunDecision::Map(_))
            && let Some(loadout) = run_loadout.take()
        {
            // A loadout the engine or the referee cannot play (a relic offered by `NeowsBones`
            // that is not ported) ends the run here, like unported content does.
            match session.apply_loadout(&loadout) {
                Ok((referee_action, line)) => {
                    actions.push(referee_action);
                    trace.push(line);
                    continue;
                }
                Err(reason) => break format!("loadout {reason}"),
            }
        }
        event.observe(session.run(), run_loadout.is_none());
        if matches!(session.run().pending_decision(), RunDecision::Map(_)) {
            match event.on_map(&mut session) {
                Ok(Some((referee_action, line))) => {
                    actions.push(referee_action);
                    trace.push(line);
                    continue;
                }
                Ok(None) => {}
                Err(stop) => break stop,
            }
        }
        if actions.len() >= max_actions {
            break "max_actions".to_owned();
        }
        if let Some(stop) = run_stop(session.run()) {
            break stop;
        }
        if let Some(action) = session.run().combat().and_then(|c| {
            if greedy {
                greedy::Greedy::answer(c)
            } else {
                whole_choice(c, |n| policy.pick(n))
            }
        }) {
            let action = RunAction::Combat(action);
            if let Err(reason) = session.check(&action) {
                break reason;
            }
            if let Some((referee_action, line)) = session.apply(&action)? {
                actions.push(referee_action);
                trace.push(line);
            }
            continue;
        }
        let action = match choose_run_action(
            &session,
            &mut policy,
            &mut planner,
            (greedy, shopper),
            &preferred,
            &mut event.first_option,
            &mut event.first_ancient_option,
        ) {
            Ok(action) => action,
            Err(reasons) => break reasons,
        };
        if let Some((referee_action, line)) = session.apply(&action)? {
            actions.push(referee_action);
            trace.push(line);
        }
    };
    let file = run_actions_file(character, ascension, act1, real_game, &actions);
    std::fs::write(args.get("--actions")?, serde_json::to_string_pretty(&file)?)?;
    write_trace(args.get("--trace")?, &trace)?;
    if let Some(path) = args.get_opt("--hits") {
        observe_effects(&session, &mut hits);
        hits.observe_trace(&trace);
        hits.write(path)?;
    }
    println!("{}", stop_line(&session, &stop));
    Ok(())
}

/// The effects of the fight the run is in, or of the one its last action ended.
fn observe_effects(session: &RunSession, hits: &mut Hits) {
    hits.observe_combat(session.run().combat());
    hits.observe_combat(session.run().ended_combat());
}

/// `run-gen`'s output line, see [`run_gen_cmd`].
fn stop_line(session: &RunSession, stop: &str) -> String {
    format!(
        "stop={stop} act={} floor={} events={} shops={} bought={} boss={}",
        session.run().state().act_index + 1,
        session.final_floor(),
        session.events_entered(),
        session.shops_entered(),
        session.purchases().map(|n| n.to_string()).join(","),
        boss_status(session.run().state(), stop == "victory"),
    )
}

/// The event `run-gen --event` forces, see [`run_gen_cmd`].
struct ForcedEvent {
    /// The event, until it is entered.
    id: Option<String>,
    entered: bool,
    /// The first-page option to take (an index into the modeled options, modulo their count).
    first_option: Option<usize>,
    /// `--ancient`: the ancient option to take (as `first_option`), until the ancient is entered.
    first_ancient_option: Option<usize>,
    /// Why the run stops once back on the map after the event: `event_done`, or `None` with the
    /// run loadout's `"continue": true` (the run goes on).
    stop_after: Option<&'static str>,
    /// `--ancient` given.
    ancient: bool,
    /// `--ancient`: whether the ancient was entered.
    ancient_seen: bool,
}

impl ForcedEvent {
    /// `--event`, else the run loadout's `"event"` (removed from the loadout, which goes to the
    /// referee's `run_loadout`); the first option is the policy seed.
    fn from_args(args: &Args, run_loadout: &mut Option<Value>) -> Result<Self> {
        let from_loadout = run_loadout
            .as_mut()
            .and_then(Value::as_object_mut)
            .and_then(|l| l.remove("event"))
            .map(|e| match e {
                Value::String(id) => Ok(id),
                _ => Err("run loadout event is not a string"),
            })
            .transpose()?;
        let id = args.get_opt("--event").map(str::to_owned).or(from_loadout);
        let stop_after = (!run_loadout
            .as_mut()
            .and_then(Value::as_object_mut)
            .and_then(|l| l.remove("continue"))
            .is_some_and(|c| c == json!(true)))
        .then_some("event_done");
        let ancient = args.switch("--ancient")?
            || run_loadout
                .as_mut()
                .and_then(Value::as_object_mut)
                .and_then(|l| l.remove("ancient"))
                .is_some_and(|a| a == json!(true));
        let policy_seed: Option<usize> = match (&id, ancient) {
            (None, false) => None,
            _ => Some(args.get("--policy-seed")?.parse()?),
        };
        Ok(Self {
            first_option: id.as_ref().and(policy_seed),
            id,
            entered: false,
            stop_after,
            first_ancient_option: policy_seed.filter(|_| ancient),
            ancient,
            ancient_seen: false,
        })
    }

    /// Notes an ancient other than Neow once the loadout is handed over (`loadout_done`).
    fn observe(&mut self, run: &Run, loadout_done: bool) {
        if loadout_done
            && matches!(run.pending_decision(), RunDecision::Ancient(room) if room.kind != AncientKind::Neow)
        {
            self.ancient_seen = true;
        }
    }

    /// The run waits on the map: the `enter_event` action and its line the first time, nothing
    /// without a forced event, `Err` with the stop reason (`event_done`, or why the event cannot
    /// be entered) after.
    fn on_map(
        &mut self,
        session: &mut RunSession,
    ) -> std::result::Result<Option<(Value, Value)>, String> {
        if self.entered {
            return self
                .stop_after
                .map_or(Ok(None), |stop| Err(stop.to_owned()));
        }
        if self.ancient && self.ancient_seen {
            return Err("ancient_done".to_owned());
        }
        let Some(id) = self.id.take() else {
            return Ok(None);
        };
        self.entered = true;
        session
            .enter_event(&id)
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

/// The act `run-gen` forces: `--act1` (the harness's `ACT1`), else the run loadout's `"act1"`, else
/// Overgrowth.
fn run_act1(args: &Args, run_loadout: Option<&Value>) -> Result<Act1> {
    args.get_opt("--act1").map_or_else(
        || run_loadout.map_or(Ok(Act1::Overgrowth), run_session::act1_of),
        |key| run_session::act1_of(&json!({ "act1": key })),
    )
}

/// The referee's run-mode actions file for `run-gen`'s `actions`.
fn run_actions_file(
    character: Character,
    ascension: u8,
    act1: Act1,
    real_game: bool,
    actions: &[Value],
) -> Value {
    let mut file = json!({
        "mode": "run",
        "character": character.name(),
        "ascension": ascension,
        "actions": actions,
    });
    if let Some(act1) = run_session::act1_field(act1) {
        file["act1"] = act1;
    }
    if real_game {
        file["real_shop_prices"] = json!(true);
        file["real_cauldron"] = json!(true);
        file["real_calling_bell"] = json!(true);
    }
    file
}

/// Why the run stopped on its own, if it has: `died`, `victory` (the last boss is beaten and the
/// run is in `TheArchitect`'s room, which `boss_status` reads), or content the engine does not port
/// (the combat's reason).
fn run_stop(run: &Run) -> Option<String> {
    match run.pending_decision() {
        RunDecision::GameOver => Some("died".to_owned()),
        RunDecision::Victory => Some("victory".to_owned()),
        RunDecision::Stopped(unsupported) => Some(unsupported.to_string()),
        _ => None,
    }
}

/// How far the run got against each act's boss, comma-separated: `beaten` (the run went on to
/// the next act, or won in the last), `reached` (a boss fight began) or `-`; then the second boss's status.
fn boss_status(s: &sts2_core::run::RunState, victory: bool) -> String {
    let last = s.acts.len() - 1;
    let second = &s.acts[last];
    let second = if second.second_boss.is_none() {
        "-"
    } else if victory {
        "beaten"
    } else if second.boss_encounters_visited > 1 {
        "reached"
    } else {
        "-"
    };
    s.acts
        .iter()
        .enumerate()
        .map(|(i, act)| {
            if s.act_index > i || (i == last && victory) {
                "beaten"
            } else if act.boss_encounters_visited > 0 {
                "reached"
            } else {
                "-"
            }
        })
        .chain(std::iter::once(second))
        .collect::<Vec<_>>()
        .join(",")
}

/// The action a run policy takes among the modeled ones; `Err` with the reasons when none is.
fn choose_run_action(
    session: &RunSession,
    policy: &mut Policy,
    planner: &mut greedy::Greedy,
    (greedy, shopper): (bool, bool),
    preferred: &[&str],
    first_event_option: &mut Option<usize>,
    first_ancient_option: &mut Option<usize>,
) -> std::result::Result<RunAction, String> {
    let mut legal = session.run().legal_actions();
    if legal.len() > 1 && !greedy {
        legal.retain(|a| *a != RunAction::Combat(Action::EndTurn));
    }
    let mut reasons = Vec::new();
    let mut modeled: Vec<RunAction> = legal
        .into_iter()
        .filter(|a| session.check(a).map_err(|r| reasons.push(r)).is_ok())
        .collect();
    // The greedy policy avoids an ancient option whose prompt offers nothing modeled.
    if greedy && matches!(session.run().pending_decision(), RunDecision::Ancient(_)) {
        let onward: Vec<RunAction> = modeled
            .iter()
            .filter(|a| session.leads_somewhere(a))
            .cloned()
            .collect();
        if !onward.is_empty() {
            modeled = onward;
        }
    }
    if modeled.is_empty() {
        reasons.sort();
        reasons.dedup();
        return Err(reasons.join(", "));
    }
    if let RunDecision::Event(_) = session.run().pending_decision()
        && let Some(index) = first_event_option.take()
    {
        let options: Vec<&RunAction> = modeled
            .iter()
            .filter(|a| matches!(a, RunAction::EventOption(_)))
            .collect();
        if !options.is_empty() {
            return Ok(options[index % options.len()].clone());
        }
    }
    if let RunDecision::Ancient(room) = session.run().pending_decision()
        && room.kind != AncientKind::Neow
        && let Some(index) = first_ancient_option.take()
    {
        let options: Vec<&RunAction> = modeled
            .iter()
            .filter(|a| matches!(a, RunAction::AncientOption(_)))
            .collect();
        if !options.is_empty() {
            return Ok(options[index % options.len()].clone());
        }
    }
    Ok(
        if shopper && let Some(action) = shop_action(session.run(), &modeled, policy) {
            action
        } else if greedy {
            planner.choose(session.run(), &modeled)
        } else if let Some(action) = preferred_action(session.run(), &modeled, preferred) {
            action
        } else {
            modeled[policy.pick(modeled.len())].clone()
        },
    )
}

/// The `shopper` policy's shopping: in a shop, leave with chance 1 in 4 at each step (always when
/// nothing is left to buy), else take a uniformly random purchase (a card, relic or potion that is
/// affordable and fits, or a card removal) among the modeled ones. `None` outside shops.
fn shop_action(run: &Run, modeled: &[RunAction], policy: &mut Policy) -> Option<RunAction> {
    if !matches!(run.pending_decision(), RunDecision::Shop(_)) {
        return None;
    }
    let purchases: Vec<&RunAction> = modeled
        .iter()
        .filter(|a| {
            matches!(
                a,
                RunAction::BuyCard(_)
                    | RunAction::BuyRelic(_)
                    | RunAction::BuyPotion(_)
                    | RunAction::RemoveCard
            )
        })
        .collect();
    if purchases.is_empty() || policy.pick(4) == 0 {
        return Some(RunAction::Proceed).filter(|a| modeled.contains(a));
    }
    Some(purchases[policy.pick(purchases.len())].clone())
}

/// With `--prefer-relics`: buying a preferred relic in a shop, Lift, Dig, Hatch, Kindle, Clone or Cook at a rest site,
/// and an event option whose callback tag is listed (`GRAB`: `EndlessConveyor`'s dishes).
fn preferred_action(run: &Run, modeled: &[RunAction], preferred: &[&str]) -> Option<RunAction> {
    if preferred.is_empty() {
        return None;
    }
    match run.pending_decision() {
        RunDecision::Shop(shop) => modeled
            .iter()
            .find(|a| {
                matches!(a, RunAction::BuyRelic(i)
                    if shop.relics[*i].id.is_some_and(|id| preferred.contains(&id)))
            })
            .cloned(),
        RunDecision::RestSite(options) => modeled
            .iter()
            .find(|a| {
                matches!(a, RunAction::RestOption(i)
                    if matches!(options[*i].kind, RestOptionKind::Lift | RestOptionKind::Dig | RestOptionKind::Hatch | RestOptionKind::Kindle | RestOptionKind::Clone | RestOptionKind::Cook))
            })
            .cloned(),
        RunDecision::Event(room) => modeled
            .iter()
            .find(|a| {
                matches!(a, RunAction::EventOption(i)
                    if room.options[*i].action.is_some_and(|o| preferred.contains(&o.tag)))
            })
            .cloned(),
        _ => None,
    }
}

/// Prints the first divergence of a run-mode trace; `Ok(false)` if there is one.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn compare_run_cmd(args: &Args) -> Result<bool> {
    let rust = read_trace(args.get("--rust")?)?;
    let referee = read_trace(args.get("--referee")?)?;
    let divergence = first_run_divergence(&rust, &referee);
    if let Some(d) = &divergence {
        println!("DIVERGENCE {d}");
    } else {
        println!("ok ({} steps)", rust.len());
    }
    Ok(divergence.is_none())
}

/// Prints the first divergence; `Ok(false)` if there is one.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn compare_cmd(args: &Args) -> Result<bool> {
    let rust = read_trace(args.get("--rust")?)?;
    let referee = read_trace(args.get("--referee")?)?;
    let divergence = first_divergence(&rust, &referee);
    if let Some(d) = &divergence {
        println!("DIVERGENCE {d}");
    } else {
        println!("ok ({} steps)", rust.len());
    }
    Ok(divergence.is_none())
}

/// Prints the Rust map of a seed in the referee's `map` JSON shape.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn map_cmd(args: &Args) -> Result<()> {
    let act = args.get_opt("--act").map_or(Ok(0), str::parse)?;
    let ascension = args.get_opt("--ascension").map_or(Ok(0), str::parse)?;
    println!(
        "{}",
        map::generate_act(args.get("--seed")?, act, ascension)?
    );
    Ok(())
}

/// Prints the first difference between the Rust map and the one in the referee's step-0 line
/// (`run.map`, or `run.act_maps[act]` with `--act`); `Ok(false)` if there is one.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn compare_map_cmd(args: &Args) -> Result<bool> {
    let seed = args.get("--seed")?;
    let referee = read_trace(args.get("--referee")?)?;
    let step0 = referee.first().ok_or("empty referee trace")?;
    let divergence = if let Some(act) = args.get_opt("--act") {
        let act = act.parse()?;
        let ascension = args.get_opt("--ascension").map_or(Ok(0), str::parse)?;
        let rust = map::generate_act(seed, act, ascension)?;
        map::first_act_divergence(&rust, step0, act)?
    } else {
        map::first_divergence(&map::generate(seed)?, step0)?
    };
    if let Some(d) = &divergence {
        println!("DIVERGENCE {d}");
    } else {
        println!("ok");
    }
    Ok(divergence.is_none())
}

/// Writes the minimal fixture for a seed's actions and the referee's trace of them.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn fixture_cmd(args: &Args) -> Result<()> {
    let actions: Value = serde_json::from_str(&std::fs::read_to_string(args.get("--actions")?)?)?;
    let referee = read_trace(args.get("--referee")?)?;
    let fixture = fixture::build(args.get("--seed")?, &actions, &referee)?;
    let out = args.get("--out")?;
    if let Some(parent) = Path::new(out).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out, serde_json::to_string_pretty(&fixture)?)?;
    println!("wrote {out}");
    Ok(())
}

/// Writes a run fixture and reports whether the engine matches it.
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn run_fixture_cmd(args: &Args) -> Result<()> {
    let actions: Value = serde_json::from_str(&std::fs::read_to_string(args.get("--actions")?)?)?;
    let referee = read_trace(args.get("--referee")?)?;
    let steps = args.get_opt("--steps").map(str::parse).transpose()?;
    let value = fixture::build_run(args.get("--seed")?, &actions, &referee, steps)?;
    let out = args.get("--out")?;
    if let Some(parent) = Path::new(out).parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Compact: run traces are long.
    std::fs::write(out, serde_json::to_string(&value)?)?;
    match fixture::RunFixture::load(Path::new(out))?.check()? {
        Some(d) => println!("wrote {out}; DIVERGENCE {d}"),
        None => println!("wrote {out}; matches"),
    }
    Ok(())
}

/// Reads the history digest on stdin and prints the engine's prediction (`history::predict`).
#[allow(clippy::print_stdout, reason = "the CLI's output is its interface")]
fn history_predict_cmd(args: &Args) -> Result<()> {
    if !args.has("--batch") {
        let input: Value = serde_json::from_reader(std::io::stdin())?;
        println!(
            "{}",
            serde_json::to_string(&sts2_diff::history::predict(&input)?)?
        );
        return Ok(());
    }
    // One JSON input per line, one result per line; a failed prediction is `{"error"}`.
    for line in std::io::stdin().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let input: Value = serde_json::from_str(&line)?;
        let result = sts2_diff::history::predict(&input)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}));
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

/// Prints the entities, options and plays the differential harness cannot check.
#[allow(clippy::print_stdout, reason = "the gap list is the command's output")]
fn oracle_gaps_cmd() -> bool {
    let gaps = run_session::oracle_gaps();
    for gap in &gaps {
        println!("{gap}");
    }
    println!("{} oracle gaps", gaps.len());
    true
}

/// Prints the sweep report; `Ok(false)` if it found an anomaly (`sweep::run`).
#[allow(
    clippy::print_stdout,
    reason = "the sweep report is the command's output"
)]
fn sweep_cmd(args: &Args) -> Result<bool> {
    let (lines, clean) = sts2_diff::sweep::run(args)?;
    for line in lines {
        println!("{line}");
    }
    Ok(clean)
}

/// `reach`: play one run with the stronger combat search and print how it ended, or replay a
/// recorded action list.
#[allow(
    clippy::print_stdout,
    reason = "the run summary is the command's output"
)]
fn reach_cmd(args: &Args) -> Result<bool> {
    if let Some(path) = args.get_opt("--replay") {
        let text = std::fs::read_to_string(path)?;
        let fixture = sts2_diff::reach::Fixture::parse(&text).map_err(Error::from)?;
        let outcome = sts2_diff::reach::replay(
            fixture.character,
            &fixture.seed,
            fixture.ascension,
            fixture.act1,
            &fixture.actions,
        )
        .map_err(Error::from)?;
        print_outcome(
            &outcome,
            &fixture.seed,
            fixture.character,
            fixture.ascension,
        );
        return Ok(outcome.end == sts2_diff::reach::End::Victory);
    }
    let seed = args.get("--seed")?;
    let character = character_or_default(args.get_opt("--character"))?;
    let ascension: u8 = args.parse_or("--ascension", 0)?;
    let act1 = match args.get_opt("--act1").unwrap_or("overgrowth") {
        "overgrowth" => Act1::Overgrowth,
        "underdocks" => Act1::Underdocks,
        "random" => Act1::Random,
        other => return Err(format!("unknown --act1 {other}").into()),
    };
    let max_actions: usize = args.parse_or("--max-actions", 60_000)?;
    let verbose = args.has("--verbose");
    let mut floor = 0;
    let played = sts2_diff::reach::play(character, seed, ascension, act1, max_actions, |run| {
        let s = run.state();
        if verbose && s.total_floor() != floor {
            floor = s.total_floor();
            println!(
                "act {} floor {} coord {:?} hp {}/{} gold {} room {:?}",
                s.act_index + 1,
                s.total_floor(),
                s.coord(),
                s.hp,
                s.max_hp,
                s.gold,
                s.room
            );
        }
    })
    .map_err(Error::from)?;
    if let Some(path) = args.get_opt("--record") {
        let text =
            sts2_diff::reach::fixture_text(character, seed, ascension, act1, &played.actions);
        std::fs::write(path, text)?;
    }
    print_outcome(&played.outcome, seed, character, ascension);
    Ok(played.outcome.end == sts2_diff::reach::End::Victory)
}

/// `reach`'s one-line summary.
#[allow(
    clippy::print_stdout,
    reason = "the run summary is the command's output"
)]
fn print_outcome(
    outcome: &sts2_diff::reach::Outcome,
    seed: &str,
    character: Character,
    ascension: u8,
) {
    println!(
        "reach: {} seed={} {} A{} act {} floor {} hp {}/{} gold {} deck {} relics {} actions {}",
        outcome.end.name(),
        seed,
        character.name(),
        ascension,
        outcome.act,
        outcome.floor,
        outcome.hp,
        outcome.max_hp,
        outcome.gold,
        outcome.deck,
        outcome.relics,
        outcome.actions
    );
}

/// The value net's use (`--value-leaves`, `--value-in`, `--value-keep`, ...) over `default`.
fn value_use_of(
    args: &Args,
    default: sts2_diff::honest::search::ValueUse,
) -> Result<sts2_diff::honest::search::ValueUse> {
    use sts2_diff::honest::search::{Scope, ValueUse};
    Ok(ValueUse {
        leaves: args.has("--value-leaves"),
        scope: match args.get_opt("--value-in") {
            None | Some("all") => Scope::All,
            Some("big") => Scope::Big,
            Some("boss") => Scope::Boss,
            Some(other) => return Err(format!("unknown --value-in {other}").into()),
        },
        keep: args.parse_or("--value-keep", default.keep)?,
        hp: args.parse_or("--value-hp", default.hp)?,
        last_boss_hp: args
            .get_opt("--value-last-boss-hp")
            .map(|_| args.parse_or("--value-last-boss-hp", 0))
            .transpose()?,
        mid_boss_hp: args
            .get_opt("--value-mid-boss-hp")
            .map(|_| args.parse_or("--value-mid-boss-hp", 0))
            .transpose()?,
        potion: args.parse_or("--value-potion", default.potion)?,
        boss_potion: args
            .get_opt("--value-boss-potion")
            .map(|_| args.parse_or("--value-boss-potion", 0))
            .transpose()?,
        road_potion: args
            .get_opt("--value-potion-road")
            .map(|_| args.parse_or("--value-potion-road", 0))
            .transpose()?,
        road_act: args.parse_or("--value-potion-road-act", default.road_act)?,
        prior: args.parse_or("--value-prior", default.prior)?,
        mix: args.parse_or("--value-mix", default.mix)?,
        mix_ends: args.parse_or("--value-mix-ends", default.mix_ends)?,
        mix_samples: args.parse_or("--value-mix-samples", default.mix_samples)?,
        mix_turns: args.parse_or("--value-mix-turns", default.mix_turns)?,
        mix_in: match args.get_opt("--value-mix-in") {
            None | Some("boss") => Scope::Boss,
            Some("big") => Scope::Big,
            Some("all") => Scope::All,
            Some(other) => return Err(format!("unknown --value-mix-in {other}").into()),
        },
    })
}

/// The `AlphaZero` tree's options (`--controller az`): its HP weight is the value search's.
fn az_config_of(
    args: &Args,
    search: &sts2_diff::honest::search::SearchConfig,
) -> Result<sts2_diff::honest::az::AzConfig> {
    use sts2_diff::honest::{az::AzConfig, search::Scope};
    let default = AzConfig::default();
    Ok(AzConfig {
        sims: args.parse_or("--sims", default.sims)?,
        big_sims: args.parse_or("--big-sims", default.big_sims)?,
        boss_sims: args.parse_or("--boss-sims", default.boss_sims)?,
        c: args.parse_or("--c", default.c)?,
        widen_k: args.parse_or("--widen-k", default.widen_k)?,
        end_k: args.parse_or("--end-k", default.end_k)?,
        widen_alpha: args.parse_or("--widen-alpha", default.widen_alpha)?,
        turns: args.parse_or("--turns", default.turns)?,
        hp: search.value.hp,
        potion: search.value.potion,
        prior_temp: args.parse_or("--prior-temp", default.prior_temp)?,
        prior_mix: args.parse_or("--prior-mix", default.prior_mix)?,
        batch: args.parse_or("--az-batch", default.batch)?,
        turn_prior: args.parse_or("--turn-prior", default.turn_prior)?,
        scope: match args.get_opt("--az-in") {
            None => default.scope,
            Some("big") => Scope::Big,
            Some("boss") => Scope::Boss,
            Some("all") => Scope::All,
            Some(other) => return Err(format!("unknown --az-in {other}").into()),
        },
    })
}

/// The `--controller` and its options (`bot`, `fights`).
fn controller_of(args: &Args) -> Result<sts2_diff::honest::Controller> {
    use sts2_diff::honest::{
        self, Controller,
        search::{Scope, SearchConfig, ValueUse},
    };
    // `--rollout FILE`, `none`, or (the tree's default) the committed weights.
    let rollout = match args.get_opt("--rollout") {
        Some("builtin") => Some(std::sync::Arc::new(honest::rollout::RolloutNet::builtin())),
        Some("v2") => Some(std::sync::Arc::new(honest::rollout::RolloutNet::v2())),
        Some(path) if path != "none" => Some(std::sync::Arc::new(
            honest::rollout::RolloutNet::load(path).map_err(Error::from)?,
        )),
        _ => None,
    };
    let builtin = || Some(std::sync::Arc::new(honest::rollout::RolloutNet::builtin()));
    Ok(match args.get_opt("--controller").unwrap_or("search") {
        "ppo" => Controller::Ppo(std::sync::Arc::new(
            honest::ppo::Net::load(args.get_opt("--weights").unwrap_or("target/combat-ppo.bin"))
                .map_err(Error::from)?,
        )),
        "search" | "az" => {
            let default = SearchConfig::default();
            let leaves_default = args.has("--value-leaves") && args.get_opt("--rollout").is_none();
            let beam = if args.has("--value-leaves") {
                ValueUse::BEAM
            } else {
                default.beam
            };
            let config = SearchConfig {
                worlds: args.parse_or("--worlds", default.worlds)?,
                big_worlds: args.parse_or("--big-worlds", default.big_worlds)?,
                boss_worlds: args.parse_or("--boss-worlds", default.boss_worlds)?,
                playouts: !args.has("--no-playouts"),
                net_playouts: args.has("--net-playouts"),
                beam: args.parse_or("--beam", beam)?,
                lookahead: args.parse_or("--lookahead", default.lookahead)?,
                beam2: args.parse_or("--beam2", default.beam2)?,
                depth: args.parse_or("--depth", default.depth)?,
                close: args.parse_or("--close", default.close)?,
                close_worlds: args.parse_or("--close-worlds", default.close_worlds)?,
                lookahead_in: match args.get_opt("--lookahead-in") {
                    None | Some("big") => Scope::Big,
                    Some("boss") => Scope::Boss,
                    Some("all") => Scope::All,
                    Some(other) => return Err(format!("unknown --lookahead-in {other}").into()),
                },
                value: value_use_of(args, leaf_default(leaves_default, default.value))?,
            };
            // `--value FILE|builtin|vn1`: the value net (RLB11; VN1's), its leaves or its playouts.
            if let Some(path) = args.get_opt("--value") {
                let net = honest::value::ValueNet::named(path).map_err(Error::from)?;
                let net = std::sync::Arc::new(net);
                if args.get_opt("--controller") == Some("az") {
                    return Ok(Controller::Az(az_config_of(args, &config)?, config, net));
                }
                let rollout = leaf_rollout(leaves_default, rollout);
                return Ok(Controller::SearchValue(config, net, rollout));
            }
            if args.get_opt("--controller") == Some("az") {
                return Err("--controller az needs --value".into());
            }
            rollout.map_or(Controller::Search(config), |net| {
                Controller::SearchRollout(config, net)
            })
        }
        "tree" => {
            let default = honest::tree::TreeConfig::default();
            Controller::Tree(
                honest::tree::TreeConfig {
                    sims: args.parse_or("--sims", default.sims)?,
                    big_sims: args.parse_or("--big-sims", default.big_sims)?,
                    boss_sims: args.parse_or("--boss-sims", default.boss_sims)?,
                    c: args.parse_or("--c", default.c)?,
                    widen_k: args.parse_or("--widen-k", default.widen_k)?,
                    widen_alpha: args.parse_or("--widen-alpha", default.widen_alpha)?,
                    playouts: !args.has("--no-playouts"),
                    playouts_all: args.has("--playouts-all"),
                    turns: args.parse_or("--turns", default.turns)?,
                    scale_playout: args.parse_or("--scale-playout", default.scale_playout)?,
                    scale_turn: args.parse_or("--scale-turn", default.scale_turn)?,
                    prior: !args.has("--no-prior"),
                    hybrid: !args.has("--no-hybrid"),
                    mode: if args.has("--plans") {
                        honest::tree::Mode::Plans
                    } else {
                        honest::tree::Mode::Atomic
                    },
                    prior_temp: args.parse_or("--prior-temp", default.prior_temp)?,
                    prior_mix: args.parse_or("--prior-mix", default.prior_mix)?,
                },
                match args.get_opt("--rollout") {
                    None => builtin(),
                    _ => rollout,
                },
            )
        }
        other => return Err(format!("unknown --controller {other}").into()),
    })
}

/// RLB14: value leaves blend the v2 playout in boss fights unless `--rollout` names one.
fn leaf_rollout(
    leaves_default: bool,
    rollout: Option<std::sync::Arc<sts2_diff::honest::rollout::RolloutNet>>,
) -> Option<std::sync::Arc<sts2_diff::honest::rollout::RolloutNet>> {
    if leaves_default {
        Some(std::sync::Arc::new(
            sts2_diff::honest::rollout::RolloutNet::v2(),
        ))
    } else {
        rollout
    }
}

/// `value` with RLB14's playout share when the v2 playout is the default.
const fn leaf_default(
    leaves_default: bool,
    value: sts2_diff::honest::search::ValueUse,
) -> sts2_diff::honest::search::ValueUse {
    use sts2_diff::honest::search::ValueUse;
    ValueUse {
        mix: if leaves_default { ValueUse::MIX } else { 0 },
        ..value
    }
}

/// The fight starts RLB6's bot meets on `--first` .. `--first + --count` (`fights`,
/// `rollout-train`), filtered by `--only`.
fn bench_starts(args: &Args) -> Result<Vec<sts2_diff::honest::FightStart>> {
    if let Some(file) = args.get_opt("--starts") {
        return recorded_starts(args, file);
    }
    if let Some(list) = args.get_opt("--seeds") {
        let seeds: Vec<u64> = std::fs::read_to_string(list)?
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        return starts_of(args, &seeds);
    }
    let first: u64 = args.get("--first")?.parse()?;
    let count: u64 = args.get("--count")?.parse()?;
    starts_in(args, first, count)
}

/// `--starts FILE`: recorded fight starts (`value-collect`'s `starts.jsonl`, each with its
/// `big` and `boss` flags), those of runs `--first` .. `--first + --count`, filtered by
/// `--only` (`boss`, `elite`, `common`, `big`; `act3` for act-3 bosses).
fn recorded_starts(args: &Args, file: &str) -> Result<Vec<sts2_diff::honest::FightStart>> {
    let first: u64 = args.get("--first")?.parse()?;
    let count: u64 = args.get("--count")?.parse()?;
    recorded_starts_in(args, file, first, count)
}

/// [`recorded_starts`] of runs `first` .. `first + count`.
fn recorded_starts_in(
    args: &Args,
    file: &str,
    first: u64,
    count: u64,
) -> Result<Vec<sts2_diff::honest::FightStart>> {
    use sts2_diff::honest::{FightStart, search::FightContext};
    let only = args.get_opt("--only");
    let mut out = Vec::new();
    for line in std::fs::read_to_string(file)?.lines() {
        let v: Value = serde_json::from_str(line)?;
        let record = sts2_rl::start::StartRecord::from_json(line)?;
        let seed: u64 = record.seed.trim_start_matches("RUN").parse()?;
        if !(first..first + count).contains(&seed) {
            continue;
        }
        let boss = v["boss"].as_bool().unwrap_or(false);
        let big = v["big"].as_bool().unwrap_or(false);
        let act = usize::try_from(record.act_index).unwrap_or(0);
        let keep = match only {
            Some("boss") => boss,
            Some("act3") => boss && act == 2,
            Some("elite") => big && !boss,
            Some("common") => !big,
            Some("big") => big,
            _ => true,
        };
        if let (true, Some(combat)) = (keep, record.combat()) {
            out.push(FightStart {
                seed,
                combat,
                context: FightContext { big, boss, act },
            });
        }
    }
    Ok(out)
}

/// [`bench_starts`] on seeds `first` .. `first + count`.
fn starts_in(args: &Args, first: u64, count: u64) -> Result<Vec<sts2_diff::honest::FightStart>> {
    let seeds: Vec<u64> = (first..first + count).collect();
    starts_of(args, &seeds)
}

/// [`bench_starts`] on `seeds`. `--only last` keeps each run's last fight (with `--every 1`,
/// the fight a lost run died in).
#[allow(
    clippy::print_stderr,
    reason = "a seed the engine panics on is reported on stderr"
)]
fn starts_of(args: &Args, seeds: &[u64]) -> Result<Vec<sts2_diff::honest::FightStart>> {
    use sts2_diff::honest::{self, Bot, Controller, search::SearchConfig};
    let every: usize = args.parse_or("--every", 4)?;
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let only: Option<&str> = args.get_opt("--only");
    // RLB6's bot exactly (8/8/32 worlds), so the starts do not move with the default; with
    // `--start-policy`, the evaluated bot (that run policy, the default search) instead, so
    // the starts are the decks and HP the real runs bring.
    // `--start-value FILE|builtin` with `--start-policy`: the RLB11 bot (value leaves) drives.
    let controller = start_controller(args.get_opt("--start-value"))?;
    let driver = run_policy_of(args.get_opt("--start-policy"))?.map_or_else(
        || Bot {
            controller: Controller::Search(SearchConfig {
                worlds: 8,
                big_worlds: 8,
                boss_worlds: 32,
                ..SearchConfig::default()
            }),
            net: None,
            run_policy: None,
        },
        |net| Bot {
            controller,
            net: None,
            run_policy: Some(net),
        },
    );
    // Filtered per seed: a start holds a whole combat, so unwanted ones are dropped at once.
    Ok(honest::on_threads(seeds, jobs, |seed| {
        // A run that panics (an engine bug) is reported and skipped.
        let mut starts = honest::caught(|| {
            Ok(honest::fight_starts(
                &driver,
                Character::Ironclad,
                seed,
                0,
                every,
            ))
        })
        .unwrap_or_else(|e| {
            eprintln!("seed {seed}: {e}");
            Vec::new()
        });
        starts.retain(|f| match only {
            Some("boss") => f.context.boss,
            Some("elite") => f.context.big && !f.context.boss,
            Some("common") => !f.context.big,
            Some("big") => f.context.big,
            _ => true,
        });
        if only == Some("last") {
            let last = starts.pop();
            starts = last.into_iter().collect();
        }
        starts
    })
    .into_iter()
    .flatten()
    .collect())
}

/// `rollout-train`: fits the learned playout policy. On the fight starts of train seeds
/// `--first` .. `--first + --count` (see `fights`), `--controller` plays every fight and the
/// policy is fitted to its moves (`--epochs`); then `--iters` REINFORCE steps of `--per-start`
/// sampled playouts from each elite and boss start. Writes the weights to `--out`.
#[allow(
    clippy::print_stdout,
    reason = "the training log is the command's output"
)]
fn rollout_train_cmd(args: &Args) -> Result<bool> {
    use sts2_diff::honest::{self, Bot, rollout};
    let first: u64 = args.get("--first")?.parse()?;
    if !(1_000_000..1_100_000).contains(&first) {
        return Err("rollout-train runs on the train seeds (1000000..1099999) only".into());
    }
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let out = args.get_opt("--out").unwrap_or("target/rlb7/rollout.bin");
    let count: u64 = args.get("--count")?.parse()?;
    let mut net = match args.get_opt("--init") {
        Some(path) => rollout::RolloutNet::load(path).map_err(Error::from)?,
        None => rollout::RolloutNet::new(),
    };
    // `--hidden N`: the v2 net (RLB14) of N hidden units, playing as the start net does.
    if let Some(hidden) = args.get_opt("--hidden") {
        net = net.widened(hidden.parse()?, 0x0123_4567);
    }
    // `--encounters`: the v3 net (RLB15), reading which elite or boss fight it is.
    if args.has("--encounters") {
        net = net.with_encounters();
    }
    let epochs: usize = args.parse_or("--epochs", 0)?;
    if epochs > 0 {
        let bot = Bot {
            controller: controller_of(args)?,
            net: None,
            run_policy: None,
        };
        // In chunks of seeds, so only one chunk's fight starts are held at a time.
        let chunk: u64 = args.parse_or("--chunk", 100)?;
        let mut samples: Vec<rollout::Sample> = Vec::new();
        let mut fights = 0;
        for at in (first..first + count).step_by(usize::try_from(chunk.max(1)).unwrap_or(1)) {
            // `--starts FILE`: recorded starts of the real bot (filtered by `--only`).
            let starts = match args.get_opt("--starts") {
                Some(file) => recorded_starts_in(args, file, at, chunk.min(first + count - at))?,
                None => starts_in(args, at, chunk.min(first + count - at))?,
            };
            fights += starts.len();
            let index: Vec<u64> = (0..starts.len() as u64).collect();
            samples.extend(
                honest::on_threads(&index, jobs, |i| {
                    let mut samples = Vec::new();
                    let _ = honest::play_fight(
                        &starts[usize::try_from(i).unwrap_or(0)],
                        &bot,
                        Some(&mut samples),
                    );
                    samples
                })
                .into_iter()
                .flatten(),
            );
        }
        let lr: f32 = args.parse_or("--lr", 0.01)?;
        let (ll, acc) = rollout::fit(&mut net, &samples, epochs, lr);
        println!(
            "distilled {} samples from {fights} fights: log-likelihood {ll:.3} accuracy {acc:.3}",
            samples.len()
        );
        std::fs::write(out, net.to_bytes())?;
    }
    // REINFORCE starts: elite and boss fights of the first `--rl-count` seeds.
    let rl_count = args.parse_or("--rl-count", count.min(240))?;
    let starts = match args.get_opt("--starts") {
        Some(file) => recorded_starts_in(args, file, first, rl_count)?,
        None => starts_in(args, first, rl_count)?,
    };
    let iters: usize = args.parse_or("--iters", 0)?;
    let per_start: usize = args.parse_or("--per-start", 16)?;
    // A fight start holds a whole combat (megabytes with its event log): the starts are moved,
    // not copied, and at most `--max-starts` are kept, evenly spaced.
    let max_starts: usize = args.parse_or("--max-starts", 1_200)?;
    let big: Vec<_> = starts.into_iter().filter(|s| s.context.big).collect();
    let big = search_positions(args, big, jobs)?;
    let stride = big.len().div_ceil(max_starts.max(1)).max(1);
    let big: Vec<_> = big.into_iter().step_by(stride).collect();
    println!("{} REINFORCE starts", big.len());
    let mut adam = rollout::Adam::new(&net, args.parse_or("--rl-lr", 0.003)?);
    // `--batch-starts N`: each step plays the next N starts (in turn), not all of them.
    let batch: usize = args.parse_or("--batch-starts", big.len())?;
    let batch = batch.clamp(1, big.len().max(1));
    let windows = (big.len() / batch).max(1);
    for it in 0..iters {
        let at = (it % windows) * batch;
        let (ret, win) = rollout::reinforce(
            &mut net,
            &mut adam,
            &big[at..(at + batch).min(big.len())],
            per_start,
            0xFEED_0000 + it as u64,
            jobs,
        );
        if it % 5 == 0 || it + 1 == iters {
            println!("iter {it}: return {ret:.3} win {win:.3}");
            std::fs::write(out, net.to_bytes())?;
        }
    }
    std::fs::write(out, net.to_bytes())?;
    Ok(true)
}

/// `rollout-train`'s REINFORCE starts: the fights of `big`, or with `--search-positions N`, N of
/// them (evenly spaced) played by `--controller` (the real bot's search with `--value builtin
/// --value-leaves`) and the positions it met at the start of each turn (RLB15).
#[allow(
    clippy::print_stdout,
    reason = "the training log is the command's output"
)]
fn search_positions(
    args: &Args,
    big: Vec<sts2_diff::honest::FightStart>,
    jobs: usize,
) -> Result<Vec<sts2_core::Combat>> {
    use sts2_diff::honest::{self, Bot};
    let n: usize = args.parse_or("--search-positions", 0)?;
    if n == 0 {
        return Ok(big.into_iter().map(|s| s.combat).collect());
    }
    let bot = Bot {
        controller: controller_of(args)?,
        net: None,
        run_policy: None,
    };
    let stride = big.len().div_ceil(n).max(1);
    let played: Vec<_> = big.into_iter().step_by(stride).collect();
    let index: Vec<u64> = (0..played.len() as u64).collect();
    let positions: Vec<_> = honest::on_threads(&index, jobs, |i| {
        let mut positions = Vec::new();
        let _ = honest::play_fight_with(
            &played[usize::try_from(i).unwrap_or(0)],
            &bot,
            None,
            Some(&mut positions),
        );
        positions
    })
    .into_iter()
    .flatten()
    .collect();
    println!("{} positions from {} fights", positions.len(), played.len());
    Ok(positions)
}

/// `rollout-eval`: the playout policy's own win rate (greedy, to the fight's end) from the elite
/// and boss starts of seeds `--first` .. `--first + --count` (held-out train seeds), each in
/// `--worlds` blindfolded worlds; `--rollout FILE`, or the priority list without one.
#[allow(
    clippy::print_stdout,
    reason = "the measurement is the command's output"
)]
fn rollout_eval_cmd(args: &Args) -> Result<bool> {
    use sts2_diff::honest::{self, rollout::RolloutNet, search::Playout, value::ValueNet};
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let worlds: u64 = args.parse_or("--worlds", 4)?;
    let starts = bench_starts(args)?;
    let index: Vec<u64> = (0..starts.len() as u64).collect();
    // `--rollout A,B,..`: each a weights file, `builtin`, `priority`, or `value|vn1` (the value
    // net's policy head); the priority list without one.
    for name in args.get_opt("--rollout").unwrap_or("priority").split(',') {
        let value = match name {
            "value" => Some(ValueNet::builtin().map_err(Error::from)?),
            "vn1" => Some(ValueNet::vn1().map_err(Error::from)?),
            _ => None,
        };
        let net = match name {
            "value" | "vn1" | "priority" => None,
            "builtin" => Some(RolloutNet::builtin()),
            "v2" => Some(RolloutNet::v2()),
            path => Some(RolloutNet::load(path).map_err(Error::from)?),
        };
        let policy = match (&value, &net) {
            (Some(value), _) => Playout::Value(value),
            (None, Some(net)) => Playout::Learned(net),
            (None, None) => Playout::Priority,
        };
        let started = std::time::Instant::now();
        let wins = honest::on_threads(&index, jobs, |i| {
            let start = &starts[usize::try_from(i).unwrap_or(0)];
            let won = (0..worlds)
                .filter(|&k| {
                    let world = start.combat.blindfold_for_search(0xE7A1 ^ (i << 8) ^ k);
                    honest::search::playout_value(world, policy) > 0
                })
                .count();
            (start.context.boss, won)
        });
        println!(
            "{name}: {} ms per playout (wall × jobs)",
            started.elapsed().as_millis() * jobs as u128
                / (starts.len() as u128 * u128::from(worlds)).max(1)
        );
        for (kind, boss) in [("elite", false), ("boss", true)] {
            let rows: Vec<_> = wins.iter().filter(|(b, _)| *b == boss).collect();
            let won: usize = rows.iter().map(|(_, w)| w).sum();
            let n = rows.len() * usize::try_from(worlds).unwrap_or(1);
            let permille = won * 1000 / n.max(1);
            println!(
                "{kind:5} starts {:4} playouts {n:5} won {won:5} rate {}.{:03}",
                rows.len(),
                permille / 1000,
                permille % 1000
            );
        }
    }
    Ok(true)
}

/// [`bench_starts`], each played `--reseed K` times (RLB16): the first as recorded and the
/// others with its hidden state (draw order, every RNG) drawn again; with the copy's index.
/// `--reseed-from J` (RLB17) skips the copies below J.
fn reseeded_starts(args: &Args) -> Result<Vec<(sts2_diff::honest::FightStart, u64)>> {
    use sts2_diff::honest::FightStart;
    let reseed: u64 = args.parse_or("--reseed", 1)?;
    let from: u64 = args.parse_or("--reseed-from", 0)?;
    Ok(bench_starts(args)?
        .into_iter()
        .flat_map(|start| {
            (from..reseed.max(1)).map(move |k| {
                let combat = if k == 0 {
                    start.combat.clone()
                } else {
                    let seed = start.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ k;
                    start.combat.blindfold_for_search(seed)
                };
                let start = FightStart {
                    seed: start.seed,
                    combat,
                    context: start.context,
                };
                (start, k)
            })
        })
        .collect())
}

/// `search-collect` (RLB17): the starts of `fights` (with `--reseed`), each played by the value
/// search; at each of its searched decisions, what the player sees and what the search
/// scored, with the fight's outcome. Writes under `--out`:
/// - `obs.bin`: the value net's observation per decision (as `value-collect`);
/// - `samples.tsv`: seed, kind, won, HP at the end, max HP, copy, turn, turn start (0/1),
///   the search's best total per world (millionths of a fight win), decision index;
/// - `pol.bin`: the playout policy's view per decision, [`POL_INTS`] i32 then [`POL_FLOATS`]
///   f32 (see [`pol_record`]).
#[allow(clippy::print_stdout, reason = "the command's summary is its output")]
fn search_collect_cmd(args: &Args) -> Result<bool> {
    use std::io::Write as _;
    use sts2_diff::honest::{self, Bot, rollout, search::Searched, value};
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let starts = reseeded_starts(args)?;
    let bot = Bot {
        controller: controller_of(args)?,
        net: None,
        run_policy: None,
    };
    let dir = args.get("--out")?;
    std::fs::create_dir_all(dir)?;
    let open = |name: &str| -> Result<_> {
        Ok(std::io::BufWriter::new(std::fs::File::create(format!(
            "{dir}/{name}"
        ))?))
    };
    let files = std::sync::Mutex::new((open("obs.bin")?, open("pol.bin")?, open("samples.tsv")?));
    let index: Vec<u64> = (0..starts.len() as u64).collect();
    let counts = honest::on_threads(&index, jobs, |i| {
        let (start, k) = &starts[usize::try_from(i).unwrap_or(0)];
        let mut combat = start.combat.clone();
        let mut scratch = rollout::Scratch::default();
        let mut rows: Vec<DecisionRow> = Vec::new();
        let mut legal = Vec::new();
        let mut turn = 0;
        for _ in 0..3_000 {
            let s = combat.state();
            if s.result.is_some() || s.player.turn_number > 60 {
                break;
            }
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            let action = match bot.fight_scored(&combat, &legal, start.context) {
                Searched::Forced(action) => action,
                Searched::Scored(scored) => {
                    let n = i64::try_from(scored.worlds.max(1)).unwrap_or(1);
                    let best = scored.totals.iter().copied().max().unwrap_or(0) / n;
                    let t = combat.state().player.turn_number;
                    let turn_start = combat.pending_decision().is_none() && t != turn;
                    if turn_start {
                        turn = t;
                    }
                    let pol = pol_record(&combat, &legal, &scored, &mut scratch);
                    rows.push(DecisionRow {
                        obs: value::observation(&combat),
                        pol,
                        turn: i32::try_from(t).unwrap_or(0),
                        turn_start,
                        best,
                    });
                    scored.best().clone()
                }
            };
            if combat.apply(action).is_err() {
                break;
            }
        }
        let s = combat.state();
        let me = s.creature(s.player.creature);
        let won = s.result == Some(sts2_core::combat::CombatResult::Won) && me.hp > 0;
        let kind = if start.context.boss {
            2
        } else {
            u8::from(start.context.big)
        };
        let mut f = files.lock().expect("no poisoned lock");
        for (d, row) in rows.iter().enumerate() {
            let _ = f.0.write_all(&row.obs);
            let _ = f.1.write_all(&row.pol);
            let _ = writeln!(
                f.2,
                "{}\t{kind}\t{}\t{}\t{}\t{k}\t{}\t{}\t{}\t{d}",
                start.seed,
                u8::from(won),
                if won { me.hp.max(0) } else { 0 },
                me.max_hp,
                row.turn,
                u8::from(row.turn_start),
                row.best
            );
        }
        drop(f);
        (rows.len(), u8::from(won))
    });
    {
        let mut f = files.lock().expect("no poisoned lock");
        f.0.flush()?;
        f.1.flush()?;
        f.2.flush()?;
    }
    let samples: usize = counts.iter().map(|c| c.0).sum();
    let won: usize = counts.iter().map(|c| usize::from(c.1)).sum();
    println!("{} fights, {won} won, {samples} decisions", counts.len());
    Ok(true)
}

/// One searched decision of `search-collect`.
struct DecisionRow {
    obs: Vec<u8>,
    pol: Vec<u8>,
    turn: i32,
    turn_start: bool,
    /// The best first action's total per world.
    best: i64,
}

/// A search total over `worlds` worlds, per world in fight wins.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    reason = "a training target, not a game rule"
)]
fn wins_per_world(total: i64, worlds: usize) -> f32 {
    (total as f64 / worlds.max(1) as f64 / 1e6) as f32
}

/// The most candidates and hand cards a policy record holds.
const POL_SLOTS: usize = 12;
/// A policy record's i32s: the candidates' count, the search's move among them (-1 when it is
/// none, a potion), the first actions searched, then [`POL_SLOTS`] candidate keys and
/// [`POL_SLOTS`] hand keys (-1 for none).
const POL_INTS: usize = 4 + 2 * POL_SLOTS;
/// A policy record's f32s: the position's features ([`sts2_diff::honest::rollout::F3`]), each
/// candidate's six features, and each candidate's search score: the best total per world of a
/// first action of its key, in fight wins (-9 when none was searched).
const POL_FLOATS: usize = sts2_diff::honest::rollout::F3 + 7 * POL_SLOTS;

/// The playout policy's view of a searched decision ([`POL_INTS`], [`POL_FLOATS`]).
fn pol_record(
    combat: &sts2_core::Combat,
    legal: &[Action],
    scored: &sts2_diff::honest::search::Scored,
    scratch: &mut sts2_diff::honest::rollout::Scratch,
) -> Vec<u8> {
    use sts2_diff::honest::rollout;
    let (f, cands, hand) = rollout::view(combat, legal, scratch);
    let key_of = |a: &Action| match a {
        Action::EndTurn => Some(0),
        Action::PlayCard { card, .. } => Some(rollout::card_key(combat, *card)),
        _ => None,
    };
    let chosen = key_of(scored.best())
        .and_then(|key| cands.iter().position(|c| c.key == key))
        .and_then(|i| i32::try_from(i).ok())
        .unwrap_or(-1);
    let mut ints = [-1_i32; POL_INTS];
    ints[0] = i32::try_from(cands.len().min(POL_SLOTS)).unwrap_or(0);
    ints[1] = chosen;
    ints[2] = i32::try_from(scored.first.len()).unwrap_or(0);
    ints[3] = 0;
    let mut floats = [0.0_f32; POL_FLOATS];
    floats[..rollout::F3].copy_from_slice(&f);
    for (j, c) in cands.iter().take(POL_SLOTS).enumerate() {
        ints[4 + j] = i32::try_from(c.key).unwrap_or(-1);
        floats[rollout::F3 + 6 * j..rollout::F3 + 6 * j + 6].copy_from_slice(&c.g);
        let score = scored
            .first
            .iter()
            .zip(&scored.totals)
            .filter(|(a, _)| key_of(a) == Some(c.key))
            .map(|(_, &t)| t)
            .max();
        let score = score.map_or(-9.0, |t| wins_per_world(t, scored.worlds));
        floats[rollout::F3 + 6 * POL_SLOTS + j] = score;
    }
    for (j, &key) in hand.iter().take(POL_SLOTS).enumerate() {
        ints[4 + POL_SLOTS + j] = i32::try_from(key).unwrap_or(-1);
    }
    ints.iter()
        .flat_map(|x| x.to_le_bytes())
        .chain(floats.iter().flat_map(|x| x.to_le_bytes()))
        .collect()
}

/// `fights`: fight starts met by RLB6's bot on seeds `--first` .. `--first + --count` (train
/// seeds; every elite and boss fight and every `--every`-th common one), each played by
/// `--controller`. Prints per fight type the fights, wins and mean HP left, and writes one line
/// per fight to `--out` for paired comparisons.
#[allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::print_stdout,
    reason = "the variants loop, the trace and the table are one command; the bench table is the command's output"
)]
fn fights_cmd(args: &Args) -> Result<bool> {
    use std::fmt::Write as _;
    use sts2_diff::honest::{self, Bot};
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let starts = reseeded_starts(args)?;
    // `--variants "w64 b24;w128;r"`: the same starts played once per variant of the search,
    // each a list of `wN` (boss worlds), `bN` (beam) and `r` (the builtin learned playout).
    let base = controller_of(args)?;
    let variants: Vec<Option<&str>> = args.get_opt("--variants").map_or_else(
        || vec![None],
        |list| list.split(';').map(|v| Some(v.trim())).collect(),
    );
    for variant in variants {
        honest::search::CLAIRVOYANT.store(false, std::sync::atomic::Ordering::Relaxed);
        honest::search::MINIONS.store(minions_of(args)?, std::sync::atomic::Ordering::Relaxed);
        let controller = match variant {
            None => base.clone(),
            Some(spec) => variant_of(&base, spec)?,
        };
        if let Some(spec) = variant {
            println!("variant {spec}");
        }
        let bot = Bot {
            controller,
            net: None,
            run_policy: None,
        };
        let started = std::time::Instant::now();
        let index: Vec<u64> = (0..starts.len() as u64).collect();
        let results = honest::on_threads(&index, jobs, |i| {
            let (start, _) = &starts[usize::try_from(i).unwrap_or(0)];
            let t = std::time::Instant::now();
            // A fight the engine panics on (a bug) is reported and counted lost.
            let (won, hp) = honest::caught(|| Ok(honest::play_fight(start, &bot, None)))
                .unwrap_or_else(|e| {
                    eprintln!("seed {} fight: {e}", start.seed);
                    (false, 0)
                });
            (won, hp, t.elapsed().as_millis())
        });
        let wall = started.elapsed().as_secs_f64();
        // `--trace DIR --trace-only ENCOUNTER`: each lost fight of that encounter, as text.
        if let Some(dir) = args.get_opt("--trace") {
            std::fs::create_dir_all(dir)?;
            let only = args.get_opt("--trace-only").unwrap_or("");
            for ((start, k), (won, _, _)) in starts.iter().zip(&results) {
                let name = format!("{:?}", start.combat.state().encounter);
                if (!won || args.has("--trace-won")) && name.contains(only) {
                    std::fs::write(
                        format!(
                            "{dir}/{}{}-{name}{}.txt",
                            start.seed,
                            if *k == 0 {
                                String::new()
                            } else {
                                format!("k{k}")
                            },
                            variant
                                .map_or_else(String::new, |v| format!(".{}", v.replace(' ', "")))
                        ),
                        trace_fight(start, &bot, args.has("--trace-advice")),
                    )?;
                }
            }
        }
        let mut lines = String::new();
        // (fights, wins, hp left, hp at start, ms) per type
        let mut table = [(0_u32, 0_u32, 0_i64, 0_i64, 0_u128); 3];
        for ((start, k), (won, hp, ms)) in starts.iter().zip(&results) {
            let kind = if start.context.boss {
                2
            } else {
                usize::from(start.context.big)
            };
            let s = start.combat.state();
            let hp0 = s.creature(s.player.creature).hp;
            let row = &mut table[kind];
            row.0 += 1;
            row.1 += u32::from(*won);
            row.2 += i64::from(*hp);
            row.3 += i64::from(hp0);
            row.4 += ms;
            let _ = writeln!(
                lines,
                "{}\t{}\t{:?}\t{}\t{}\t{hp}\t{ms}\t{k}",
                start.seed,
                ["common", "elite", "boss"][kind],
                start.combat.state().encounter,
                u8::from(*won),
                hp0
            );
        }
        for (name, (n, w, hp, hp0, ms)) in ["common", "elite", "boss"].iter().zip(table) {
            let n1 = i64::from(n.max(1));
            // Means in tenths, in integers.
            let (left, lost) = (hp * 10 / n1, (hp0 - hp) * 10 / n1);
            println!(
                "{name:7} fights {n:4} wins {w:4} hp left {:4}.{} lost {:4}.{} ms/fight {}",
                left / 10,
                left % 10,
                lost / 10,
                lost % 10,
                ms / u128::from(n.max(1))
            );
        }
        println!("wall {wall:.1}s");
        if let Some(out) = args.get_opt("--out") {
            let out = variant.map_or_else(
                || out.to_owned(),
                |v| format!("{out}.{}", v.replace(' ', "")),
            );
            std::fs::write(out, lines)?;
        }
    }
    Ok(true)
}

/// `az-collect`: the fight starts of `fights` (filtered by `--only`), each played by the
/// `AlphaZero` tree (`--controller az`, in its scope; elsewhere the value search). Writes under
/// `--out` each tree decision's observation (`obs.bin`, as the value net reads it), its root
/// visits by action index (`visits.bin`, f32, -1 where the action has no edge) and
/// `samples.tsv` (seed, kind, won, HP at the end, max HP): expert-iteration targets.
fn az_collect_cmd(args: &Args) -> Result<bool> {
    use std::io::Write as _;
    use sts2_diff::honest::{self, Bot, Controller, az, value};
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let starts = bench_starts(args)?;
    let controller = controller_of(args)?;
    let Controller::Az(config, _, net) = &controller else {
        return Err("az-collect needs --controller az".into());
    };
    let (config, net) = (*config, net.clone());
    let bot = Bot {
        controller: controller.clone(),
        net: None,
        run_policy: None,
    };
    let dir = args.get("--out")?;
    std::fs::create_dir_all(dir)?;
    let open = |name: &str| -> Result<_> {
        Ok(std::io::BufWriter::new(std::fs::File::create(format!(
            "{dir}/{name}"
        ))?))
    };
    let files =
        std::sync::Mutex::new((open("obs.bin")?, open("visits.bin")?, open("samples.tsv")?));
    let index: Vec<u64> = (0..starts.len() as u64).collect();
    let counts = honest::on_threads(&index, jobs, |i| {
        let start = &starts[usize::try_from(i).unwrap_or(0)];
        let mut combat = start.combat.clone();
        let (mut obs, mut visits, mut decisions) = (Vec::new(), Vec::new(), 0_usize);
        let mut legal = Vec::new();
        for _ in 0..3_000 {
            let s = combat.state();
            if s.result.is_some() || s.player.turn_number > 60 {
                break;
            }
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            let action = if config.scope.holds(start.context) {
                let root = az::search_root(&combat, &legal, start.context, config, &net);
                if root.len() > 1 {
                    obs.extend_from_slice(&value::observation(&combat));
                    let mut v = vec![-1.0_f32; sts2_rl::ACTIONS];
                    for (a, n, _) in &root {
                        if let Some(k) = sts2_rl::action::index_of(&combat, a) {
                            v[k] = f32::from(u16::try_from(*n).unwrap_or(u16::MAX));
                        }
                    }
                    visits.extend(v.iter().flat_map(|x| x.to_le_bytes()));
                    decisions += 1;
                }
                az::best(&root).unwrap_or_else(|| legal[0].clone())
            } else {
                bot.fight_action(&combat, &legal, start.context)
            };
            if combat.apply(action).is_err() {
                break;
            }
        }
        let s = combat.state();
        let me = s.creature(s.player.creature);
        let won = s.result == Some(sts2_core::combat::CombatResult::Won) && me.hp > 0;
        let kind = if start.context.boss {
            2
        } else {
            u8::from(start.context.big)
        };
        let row = format!(
            "{}\t{kind}\t{}\t{}\t{}\n",
            start.seed,
            u8::from(won),
            me.hp.max(0),
            me.max_hp
        );
        let mut f = files.lock().expect("no poisoned lock");
        let _ = f.0.write_all(&obs);
        let _ = f.1.write_all(&visits);
        let _ = f.2.write_all(row.repeat(decisions).as_bytes());
        drop(f);
        (decisions, u8::from(won))
    });
    {
        let mut f = files.lock().expect("no poisoned lock");
        f.0.flush()?;
        f.1.flush()?;
        f.2.flush()?;
    }
    let samples: usize = counts.iter().map(|c| c.0).sum();
    let won: usize = counts.iter().map(|c| usize::from(c.1)).sum();
    #[allow(clippy::print_stdout, reason = "the command's summary")]
    {
        println!("{} fights, {won} won, {samples} samples", counts.len());
    }
    Ok(true)
}

/// A trace's state line: the turn, the player's HP, block and belt, and each living enemy's
/// HP, block, powers and intents.
fn trace_line(s: &sts2_core::combat::CombatState, out: &mut String) {
    use std::fmt::Write as _;
    let p = s.creature(s.player.creature);
    let belt: Vec<String> = s
        .player
        .potions
        .iter()
        .flatten()
        .map(|p| format!("{:?}", p.kind))
        .collect();
    let _ = write!(
        out,
        "T{} hp {} blk {} belt [{}] |",
        s.player.turn_number,
        p.hp,
        p.block,
        belt.join(",")
    );
    for &id in &s.enemies {
        let c = s.creature(id);
        let powers: Vec<String> = c
            .powers
            .iter()
            .map(|p| format!("{}{}", p.kind.name(), p.amount))
            .collect();
        if c.hp <= 0 {
            continue;
        }
        let intents: Vec<String> = s
            .intents(id)
            .iter()
            .map(|i| match i.attack {
                Some(a) if a.hits > 1 => format!("{:?} {}x{}", i.kind, a.damage, a.hits),
                Some(a) => format!("{:?} {}", i.kind, a.damage),
                None => format!("{:?}", i.kind),
            })
            .collect();
        let _ = write!(
            out,
            " [{} {}/{} b{} {} <{}>]",
            id.0,
            c.hp,
            c.max_hp,
            c.block,
            powers.join(","),
            intents.join(",")
        );
    }
    out.push('\n');
}

/// One fight played by `bot` again, as text: each action with the player's and the enemies'
/// HP, block and powers before it (`fights --trace`).
fn trace_fight(
    start: &sts2_diff::honest::FightStart,
    bot: &sts2_diff::honest::Bot,
    advice: bool,
) -> String {
    use std::fmt::Write as _;
    use sts2_core::combat::Action;
    let mut combat = start.combat.clone();
    let mut out = String::new();
    let mut legal = Vec::new();
    let mut last_turn = 0;
    for _ in 0..3_000 {
        if combat.state().result.is_some() || combat.state().player.turn_number > 60 {
            break;
        }
        combat.legal_actions_into(&mut legal);
        if legal.is_empty() {
            break;
        }
        let action = bot.fight_action(&combat, &legal, start.context);
        // `--trace-advice`: what the clairvoyant search (a diagnostic) plays here instead.
        let seer = advice.then(|| {
            use std::sync::atomic::Ordering::Relaxed;
            let was = sts2_diff::honest::search::CLAIRVOYANT.swap(true, Relaxed);
            let seer = bot.fight_action(&combat, &legal, start.context);
            sts2_diff::honest::search::CLAIRVOYANT.store(was, Relaxed);
            seer
        });
        let s = combat.state();
        if s.player.turn_number != last_turn {
            last_turn = s.player.turn_number;
            out.push_str("start ");
            trace_line(s, &mut out);
            let hand: Vec<&str> = s
                .player
                .pile(sts2_core::combat::PileType::Hand)
                .iter()
                .map(|&c| s.card(c).kind.name())
                .collect();
            let _ = writeln!(
                out,
                "  hand (energy {}, draw {}, discard {}): {}",
                s.player.energy,
                s.player.pile(sts2_core::combat::PileType::Draw).len(),
                s.player.pile(sts2_core::combat::PileType::Discard).len(),
                hand.join(", ")
            );
        }
        if matches!(action, Action::EndTurn) {
            trace_line(s, &mut out);
        }
        let desc = |action: &Action| match action {
            Action::PlayCard { card, target } => {
                format!(
                    "play {} -> {:?}",
                    s.card(*card).kind.name(),
                    target.map(|t| t.0)
                )
            }
            other => format!("{other:?}"),
        };
        let _ = write!(out, "  {}", desc(&action));
        if let Some(seer) = seer.filter(|seer| *seer != action) {
            let _ = write!(out, "    [clairvoyant: {}]", desc(&seer));
        }
        out.push('\n');
        if combat.apply(action).is_err() {
            break;
        }
    }
    trace_line(combat.state(), &mut out);
    out
}

/// A search controller from `base` with the overrides of `spec` (see `fights --variants`).
fn variant_of(
    base: &sts2_diff::honest::Controller,
    spec: &str,
) -> Result<sts2_diff::honest::Controller> {
    use sts2_diff::honest::{Controller, rollout::RolloutNet};
    if let Controller::Az(c, search, value) = base {
        let mut config = *c;
        for part in spec.split_whitespace() {
            match part.split_at(1) {
                ("s", n) => config.sims = n.parse()?,
                ("S", n) => config.big_sims = n.parse()?,
                ("B", n) => config.boss_sims = n.parse()?,
                ("c", n) => config.c = n.parse()?,
                ("k", n) => config.widen_k = n.parse()?,
                ("e", n) => config.end_k = n.parse()?,
                ("a", n) => config.widen_alpha = n.parse()?,
                ("t", n) => config.turns = n.parse()?,
                ("m", n) => config.prior_mix = n.parse()?,
                ("T", n) => config.prior_temp = n.parse()?,
                ("i", "big") => config.scope = sts2_diff::honest::search::Scope::Big,
                ("i", "boss") => config.scope = sts2_diff::honest::search::Scope::Boss,
                ("i", "all") => config.scope = sts2_diff::honest::search::Scope::All,
                ("b", n) => config.batch = n.parse()?,
                ("h", n) => config.turn_prior = n.parse()?,
                _ => return Err(format!("unknown variant part {part}").into()),
            }
        }
        return Ok(Controller::Az(config, *search, value.clone()));
    }
    if let Controller::SearchValue(c, value, rollout) = base {
        let mut config = *c;
        for part in spec.split_whitespace() {
            match part.split_at(1) {
                ("w", n) => config.boss_worlds = n.parse()?,
                ("b", n) => config.beam = n.parse()?,
                ("l", n) => config.lookahead = n.parse()?,
                ("k", n) => config.value.keep = n.parse()?,
                ("h", n) => config.value.hp = n.parse()?,
                ("p", n) => config.value.potion = n.parse()?,
                ("R", n) => config.value.road_potion = Some(n.parse()?),
                ("P", n) => config.value.prior = n.parse()?,
                ("W", n) => config.big_worlds = n.parse()?,
                ("v", "") => config.value.leaves = true,
                ("n", "") => config.value.leaves = false,
                ("i", "big") => config.value.scope = sts2_diff::honest::search::Scope::Big,
                ("i", "boss") => config.value.scope = sts2_diff::honest::search::Scope::Boss,
                ("a", "") => config.lookahead_in = sts2_diff::honest::search::Scope::All,
                ("o", "") => config.lookahead_in = sts2_diff::honest::search::Scope::Boss,
                ("x", "") => config.playouts = false,
                ("m", n) => config.value.mix = n.parse()?,
                ("e", n) => config.value.mix_ends = n.parse()?,
                ("s", n) => config.value.mix_samples = n.parse()?,
                ("t", n) => config.value.mix_turns = n.parse()?,
                // Clairvoyant (a diagnostic ceiling): the search sees the real world.
                ("c", "") => sts2_diff::honest::search::CLAIRVOYANT
                    .store(true, std::sync::atomic::Ordering::Relaxed),
                // RLB16: how the turn score counts a Minion (0 kills, 1 HP, 2 primary only).
                ("M", n) => sts2_diff::honest::search::MINIONS
                    .store(n.parse()?, std::sync::atomic::Ordering::Relaxed),
                _ => return Err(format!("unknown variant part {part}").into()),
            }
        }
        return Ok(Controller::SearchValue(
            config,
            value.clone(),
            rollout.clone(),
        ));
    }
    let (mut config, mut net) = match base {
        Controller::Search(c) => (*c, None),
        Controller::SearchRollout(c, net) => (*c, Some(net.clone())),
        _ => return Err("--variants needs the search controller".into()),
    };
    for part in spec.split_whitespace() {
        match part.split_at(1) {
            ("w", n) => config.boss_worlds = n.parse()?,
            ("b", n) => config.beam = n.parse()?,
            ("l", n) => config.lookahead = n.parse()?,
            ("B", n) => config.beam2 = n.parse()?,
            ("d", n) => config.depth = n.parse()?,
            ("a", "") => config.lookahead_in = sts2_diff::honest::search::Scope::All,
            ("g", n) => config.close = n.parse()?,
            ("x", n) => config.close_worlds = n.parse()?,
            ("o", "") => config.lookahead_in = sts2_diff::honest::search::Scope::Boss,
            ("r", "") => net = Some(std::sync::Arc::new(RolloutNet::builtin())),
            // Clairvoyant (a diagnostic ceiling): the search sees the real world.
            ("c", "") => sts2_diff::honest::search::CLAIRVOYANT
                .store(true, std::sync::atomic::Ordering::Relaxed),
            _ => return Err(format!("unknown variant part {part}").into()),
        }
    }
    Ok(net.map_or(Controller::Search(config), |net| {
        Controller::SearchRollout(config, net)
    }))
}

/// `bot`: the honest bot (`sts2_diff::honest`) on seeds `--first` .. `--first + --count`. Prints
/// one line per seed and the summary, writes them and each win's replay fixture under `--out`,
/// and replays every win; `Ok(false)` if a win does not replay. With `--measure-options` it
/// measures event and ancient options instead (train seeds only) and writes the value table.
#[allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::print_stdout,
    reason = "one command with its modes; the batch rows and summary are the command's output"
)]
fn bot_cmd(args: &Args) -> Result<bool> {
    use std::fmt::Write as _;
    use sts2_diff::honest::{self, Bot};
    let first: u64 = args.get("--first")?.parse()?;
    let count: u64 = args.get("--count")?.parse()?;
    let character = Character::from_name(args.get_opt("--character").unwrap_or("Ironclad"))
        .ok_or("unknown --character")?;
    let ascension: u8 = args.parse_or("--ascension", 0)?;
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let out = args.get_opt("--out").unwrap_or("target/honest");
    let controller = controller_of(args)?;
    if args.has("--clairvoyant-fights") {
        // A ceiling diagnostic, not the honest bot: the search sees the real world.
        eprintln!("CLAIRVOYANT fights: a ceiling, not an honest result");
        honest::search::CLAIRVOYANT.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    honest::search::MINIONS.store(minions_of(args)?, std::sync::atomic::Ordering::Relaxed);
    honest::CURSE_REMOVAL.store(
        args.has("--remove-curses"),
        std::sync::atomic::Ordering::Relaxed,
    );
    honest::search::SEARCH_SALT.store(
        args.parse_or("--search-salt", 0)?,
        std::sync::atomic::Ordering::Relaxed,
    );
    // RLB18: `--deck-probe WORLDS,MARGIN,WEIGHT` re-ranks close run decisions by the deck probe.
    if let Some(spec) = args.get_opt("--deck-probe") {
        let parts: Vec<&str> = spec.split(',').collect();
        let [worlds, margin, weight] = parts.as_slice() else {
            return Err("--deck-probe takes WORLDS,MARGIN,WEIGHT".into());
        };
        let config = honest::probe::ProbeUse {
            worlds: worlds.parse()?,
            margin: margin.parse()?,
            weight: weight.parse()?,
        };
        let _ = honest::probe::PROBE.set((config, honest::rollout::RolloutNet::v2()));
    }
    // The net also drives the search's playouts when they are asked for.
    let net = honest::ppo::Net::load(args.get_opt("--weights").unwrap_or("target/combat-ppo.bin"))
        .ok()
        .map(std::sync::Arc::new);
    let bot = Bot {
        controller,
        net,
        run_policy: run_policy_of(args.get_opt("--run-policy"))?,
    };
    // `--seeds FILE` (whitespace-separated) replaces the range, for diagnostics on chosen seeds.
    let seeds: Vec<u64> = match args.get_opt("--seeds") {
        Some(list) => std::fs::read_to_string(list)?
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?,
        None => (first..first + count).collect(),
    };
    std::fs::create_dir_all(out)?;
    let started = std::time::Instant::now();
    if args.has("--measure-options") {
        if !(1_000_000..1_100_000).contains(&first) || first + count > 1_100_000 {
            return Err("--measure-options runs on the train seeds (1000000..1099999) only".into());
        }
        let samples: Vec<_> = honest::on_threads(&seeds, jobs, |seed| {
            honest::measure_options(&bot, character, seed, ascension)
        })
        .into_iter()
        .flatten()
        .collect();
        let path = format!("{out}/table.rs");
        std::fs::write(&path, honest::table_source(&samples))?;
        println!(
            "{} samples in {:.1}s -> {path}",
            samples.len(),
            started.elapsed().as_secs_f64()
        );
        return Ok(true);
    }
    // Each row is appended to `progress.tsv` as its run ends, so a long batch shows progress.
    let progress = std::sync::Mutex::new(std::fs::File::create(format!("{out}/progress.tsv"))?);
    let results = honest::on_threads(&seeds, jobs, |seed| {
        // A run the engine panics on is an error row, not the end of the batch.
        let result = honest::caught(|| honest::play(&bot, character, seed, ascension));
        if let Ok(played) = &result {
            use std::io::Write as _;
            let line = honest::row_line(played);
            let _ = writeln!(progress.lock().expect("no poisoned lock"), "{line}");
        }
        result
    });
    let wall = started.elapsed().as_secs_f64();
    let mut rows = Vec::new();
    let mut lines = String::new();
    let mut errors = 0;
    for (seed, result) in seeds.iter().zip(results) {
        match result {
            Ok(played) => {
                lines.push_str(&honest::row_line(&played));
                lines.push('\n');
                rows.push(played);
            }
            Err(e) => {
                errors += 1;
                let _ = writeln!(lines, "{}\terror {e}", honest::seed_name(*seed));
            }
        }
    }
    let wins_dir = format!("{out}/wins");
    std::fs::create_dir_all(&wins_dir)?;
    // Every other run's actions too (RLB16), for `run-log` and replays of the losses.
    let losses_dir = format!("{out}/losses");
    std::fs::create_dir_all(&losses_dir)?;
    for played in rows.iter().filter(|p| p.end != honest::End::Victory) {
        let name = honest::seed_name(played.seed);
        let text = sts2_diff::reach::fixture_text(
            character,
            &name,
            ascension,
            Act1::Random,
            &played.actions,
        );
        std::fs::write(format!("{losses_dir}/{name}.txt"), text)?;
    }
    let mut replayed = 0;
    for played in rows.iter().filter(|p| p.end == honest::End::Victory) {
        let name = honest::seed_name(played.seed);
        let text = sts2_diff::reach::fixture_text(
            character,
            &name,
            ascension,
            Act1::Random,
            &played.actions,
        );
        std::fs::write(format!("{wins_dir}/{name}.txt"), text)?;
        if honest::replays(played, character, ascension) {
            replayed += 1;
        } else {
            let _ = writeln!(lines, "{name}\tWIN DOES NOT REPLAY");
        }
    }
    let summary = honest::summary(&rows, replayed, errors, wall);
    #[allow(clippy::print_stderr, reason = "the probe's use is a diagnostic")]
    if honest::probe::PROBE.get().is_some() {
        let ordering = std::sync::atomic::Ordering::Relaxed;
        eprintln!(
            "deck probe: {} close decisions probed, {} changed",
            honest::probe::PROBED.load(ordering),
            honest::probe::FLIPPED.load(ordering)
        );
    }
    print!("{lines}{summary}");
    std::fs::write(format!("{out}/rows.tsv"), &lines)?;
    std::fs::write(format!("{out}/summary.txt"), &summary)?;
    let wins = rows
        .iter()
        .filter(|p| p.end == honest::End::Victory)
        .count();
    Ok(replayed == wins)
}

/// `--minions kills|hp|primary` ([`sts2_diff::honest::search::MinionScoring`]; default kills).
fn minions_of(args: &Args) -> Result<u8> {
    use sts2_diff::honest::search::MinionScoring;
    Ok(match args.get_opt("--minions") {
        None | Some("kills") => MinionScoring::Kills,
        Some("hp") => MinionScoring::Hp,
        Some("primary") => MinionScoring::Primary,
        Some(other) => return Err(format!("unknown --minions {other}").into()),
    } as u8)
}

/// `run-log --dirs A,B --out OUT.jsonl`: each run fixture in the directories (`bot`'s `wins/`
/// and `losses/`) replayed as JSON lines ([`sts2_diff::honest::log`]), each tagged with its seed.
#[allow(
    clippy::print_stderr,
    reason = "a fixture that does not replay is reported"
)]
fn run_log_cmd(args: &Args) -> Result<bool> {
    use std::io::Write as _;
    let mut out = std::io::BufWriter::new(std::fs::File::create(args.get("--out")?)?);
    let mut failed = 0;
    let mut paths = Vec::new();
    for dir in args.get("--dirs")?.split(',') {
        for entry in std::fs::read_dir(dir)? {
            paths.push(entry?.path());
        }
    }
    paths.sort();
    for path in &paths {
        let fixture = sts2_diff::reach::Fixture::parse(&std::fs::read_to_string(path)?)?;
        match sts2_diff::honest::log::run_log(&fixture) {
            Ok(lines) => {
                for line in lines.lines() {
                    writeln!(out, "{{\"seed\":\"{}\",{}", fixture.seed, &line[1..])?;
                }
            }
            Err(e) => {
                failed += 1;
                eprintln!("{}: {e}", path.display());
            }
        }
    }
    Ok(failed == 0)
}

/// Each run fixture in the comma-separated directories `--dirs`, replayed on `--jobs` threads
/// from a new run: `f` sees the fixture's seed and the run before each action, whether a fight
/// starts there, and appends its lines.
/// The lines are written to `--out` in file order; `Ok(false)` if a fixture did not replay.
#[allow(
    clippy::print_stderr,
    reason = "a fixture that does not replay is reported"
)]
fn on_fixtures(
    args: &Args,
    header: &str,
    f: impl Fn(&str, &Run, bool, &mut String) + Sync,
) -> Result<bool> {
    use sts2_core::run::{Run, UnlockState};
    let mut paths = Vec::new();
    for dir in args.get("--dirs")?.split(',') {
        for entry in std::fs::read_dir(dir)? {
            paths.push(entry?.path());
        }
    }
    paths.sort();
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let index: Vec<u64> = (0..paths.len() as u64).collect();
    let rows = sts2_diff::honest::on_threads(&index, jobs, |i| {
        let path = &paths[usize::try_from(i).unwrap_or(0)];
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let fixture = sts2_diff::reach::Fixture::parse(&text)?;
        let mut run = Run::new(
            fixture.character,
            &fixture.seed,
            fixture.ascension,
            UnlockState::all(),
            fixture.act1,
        )
        .map_err(|e| e.to_string())?
        .with_combat_start_streams();
        let mut out = String::new();
        let mut in_fight = false;
        for action in &fixture.actions {
            let fighting = run.combat().is_some();
            f(&fixture.seed, &run, fighting && !in_fight, &mut out);
            in_fight = fighting;
            run.apply(action.clone()).map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(out)
    });
    let mut text = String::from(header);
    let mut failed = 0;
    for row in rows {
        match row {
            Ok(r) => text.push_str(&r),
            Err(e) => {
                failed += 1;
                eprintln!("{e}");
            }
        }
    }
    std::fs::write(args.get("--out")?, text)?;
    Ok(failed == 0)
}

/// Whether `run` is in a boss room.
const fn in_boss_room(run: &Run) -> bool {
    use sts2_core::run::{Room, RoomType};
    matches!(
        run.state().room,
        Room::Combat {
            room_type: RoomType::Boss,
            ..
        }
    )
}

/// `boss-starts` (RLB18): the start record of every boss fight of the finished runs in `--dirs`
/// (fixtures), replayed, as `collect-starts` writes them (`fights --starts`), to `--out`.
fn boss_starts_cmd(args: &Args) -> Result<bool> {
    on_fixtures(args, "", |seed, run, start, out| {
        if start
            && in_boss_room(run)
            && let Some(record) = sts2_rl::StartRecord::from_run(run, seed.to_owned())
            && let Ok(mut v) = serde_json::from_str::<Value>(&record.to_json())
        {
            v["big"] = true.into();
            v["boss"] = true.into();
            out.push_str(&v.to_string());
            out.push('\n');
        }
    })
}

/// `deck-probe` (RLB18): every boss fight start of the runs in `--dirs` (fixtures), probed by
/// `--worlds` (200) playouts of the boss by `--rollout` (v2): one row each of the seed, act,
/// boss, playouts won, playouts, mean HP left in the won ones, HP and max HP.
fn deck_probe_cmd(args: &Args) -> Result<bool> {
    use std::fmt::Write as _;
    use sts2_diff::honest::{probe, rollout::RolloutNet};
    let worlds: u64 = args.parse_or("--worlds", 200)?;
    let net = match args.get_opt("--rollout").unwrap_or("v2") {
        "v2" => RolloutNet::v2(),
        path => RolloutNet::load(path).map_err(Error::from)?,
    };
    on_fixtures(
        args,
        "seed\tact\tboss\twon\tn\thp_left\thp\tmax_hp\n",
        |seed, run, start, out| {
            if start && in_boss_room(run) {
                let s = run.state();
                let p = probe::probe(s, worlds, &net);
                let _ = writeln!(
                    out,
                    "{seed}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    s.act_index + 1,
                    s.act().next_boss_encounter().id,
                    p.won,
                    p.n,
                    p.hp_left / i64::from(p.won.max(1)),
                    s.hp,
                    s.max_hp
                );
            }
        },
    )
}

/// `collect-starts`: the start record of every fight the bot (the default search, the run
/// policy `--run-policy`) meets on train seeds `--first` .. `--first + --count`, one JSON line
/// each with `"big"` and `"boss"` added, written to `--out` (`RLB11`).
#[allow(
    clippy::print_stdout,
    reason = "the count of records is the command's output"
)]
fn collect_starts_cmd(args: &Args) -> Result<bool> {
    use std::io::Write as _;
    use sts2_diff::honest::{self, Bot};
    let first: u64 = args.get("--first")?.parse()?;
    let count: u64 = args.get("--count")?.parse()?;
    if !(1_000_000..1_100_000).contains(&first) || first + count > 1_100_000 {
        return Err("collect-starts runs on the train seeds (1000000..1099999) only".into());
    }
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let bot = Bot {
        controller: start_controller(args.get_opt("--start-value"))?,
        net: None,
        run_policy: run_policy_of(args.get_opt("--run-policy"))?,
    };
    let out = std::sync::Mutex::new(std::io::BufWriter::new(std::fs::File::create(
        args.get("--out")?,
    )?));
    // `--seeds FILE` (train seeds, whitespace-separated) replaces the range.
    let seeds: Vec<u64> = match args.get_opt("--seeds") {
        Some(list) => std::fs::read_to_string(list)?
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?,
        None => (first..first + count).collect(),
    };
    if seeds.iter().any(|s| !(1_000_000..1_100_000).contains(s)) {
        return Err("collect-starts runs on the train seeds (1000000..1099999) only".into());
    }
    let counts = honest::on_threads(&seeds, jobs, |seed| {
        let (records, mismatched) =
            honest::caught(|| Ok(honest::fight_records(&bot, Character::Ironclad, seed, 0)))
                .unwrap_or_default();
        let mut lines = String::new();
        for (record, context) in &records {
            let mut v: Value = serde_json::from_str(&record.to_json()).expect("a record is JSON");
            v["big"] = context.big.into();
            v["boss"] = context.boss.into();
            lines.push_str(&v.to_string());
            lines.push('\n');
        }
        let _ = out
            .lock()
            .expect("no poisoned lock")
            .write_all(lines.as_bytes());
        (records.len(), mismatched)
    });
    out.lock().expect("no poisoned lock").flush()?;
    let records: usize = counts.iter().map(|c| c.0).sum();
    let mismatched: usize = counts.iter().map(|c| c.1).sum();
    println!("{records} records, {mismatched} fights whose record differs (left out)");
    Ok(true)
}

/// `value-collect`: as `collect-starts` (to `--out`/`starts.jsonl`), and the observation at
/// the start of each of the bot's turns with the fight's outcome under its play (RLB12's
/// expert-iteration data): `obs.bin` (rows of [`sts2_diff::honest::value::observation`]) and
/// `samples.tsv` (seed, kind 0 common / 1 elite / 2 boss, won, HP at the end, max HP).
#[allow(
    clippy::print_stdout,
    reason = "the count of records is the command's output"
)]
fn value_collect_cmd(args: &Args) -> Result<bool> {
    use std::fmt::Write as _;
    use std::io::Write as _;
    use sts2_diff::honest::{self, Bot};
    let first: u64 = args.get("--first")?.parse()?;
    let count: u64 = args.get("--count")?.parse()?;
    if !(1_000_000..1_100_000).contains(&first) || first + count > 1_100_000 {
        return Err("value-collect runs on the train seeds (1000000..1099999) only".into());
    }
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let bot = Bot {
        controller: start_controller(args.get_opt("--start-value"))?,
        net: None,
        run_policy: run_policy_of(args.get_opt("--run-policy"))?,
    };
    let dir = args.get("--out")?;
    std::fs::create_dir_all(dir)?;
    let open = |name: &str| -> Result<_> {
        Ok(std::sync::Mutex::new(std::io::BufWriter::new(
            std::fs::File::create(format!("{dir}/{name}"))?,
        )))
    };
    let (starts, obs, table) = (
        open("starts.jsonl")?,
        open("obs.bin")?,
        open("samples.tsv")?,
    );
    let seeds: Vec<u64> = (first..first + count).collect();
    let counts = honest::on_threads(&seeds, jobs, |seed| {
        let (records, samples) =
            honest::caught(|| Ok(honest::value_records(&bot, Character::Ironclad, seed, 0)))
                .unwrap_or_default();
        let mut lines = String::new();
        for (record, context) in &records {
            let mut v: Value = serde_json::from_str(&record.to_json()).expect("a record is JSON");
            v["big"] = context.big.into();
            v["boss"] = context.boss.into();
            lines.push_str(&v.to_string());
            lines.push('\n');
        }
        let mut rows = String::new();
        let mut bytes = Vec::new();
        for sample in &samples {
            let kind = if sample.context.boss {
                2
            } else {
                u8::from(sample.context.big)
            };
            let _ = writeln!(
                rows,
                "{seed}\t{kind}\t{}\t{}\t{}",
                u8::from(sample.won),
                sample.hp,
                sample.max_hp
            );
            bytes.extend_from_slice(&sample.obs);
        }
        // One lock order (starts, then the samples' two files together), so rows stay paired.
        let _ = starts
            .lock()
            .expect("no poisoned lock")
            .write_all(lines.as_bytes());
        let mut obs = obs.lock().expect("no poisoned lock");
        let mut table = table.lock().expect("no poisoned lock");
        let _ = obs.write_all(&bytes);
        drop(obs);
        let _ = table.write_all(rows.as_bytes());
        drop(table);
        (records.len(), samples.len())
    });
    for file in [&starts, &obs, &table] {
        file.lock().expect("no poisoned lock").flush()?;
    }
    let records: usize = counts.iter().map(|c| c.0).sum();
    let samples: usize = counts.iter().map(|c| c.1).sum();
    println!("{records} start records, {samples} turn samples");
    Ok(true)
}

/// The fights of a driving bot: the default search, or with a value net at `value` the honest
/// bot's search (`--value-leaves --value-hp 300`, beam [`ValueUse::BEAM`]).
fn start_controller(value: Option<&str>) -> Result<sts2_diff::honest::Controller> {
    use sts2_diff::honest::{
        Controller,
        search::{SearchConfig, ValueUse},
        value::ValueNet,
    };
    let Some(path) = value else {
        return Ok(Controller::Search(SearchConfig::default()));
    };
    let net = ValueNet::named(path).map_err(Error::from)?;
    let config = SearchConfig {
        value: ValueUse {
            leaves: true,
            hp: 300,
            mix: ValueUse::MIX,
            ..ValueUse::DEFAULT
        },
        beam: ValueUse::BEAM,
        ..SearchConfig::default()
    };
    Ok(Controller::SearchValue(
        config,
        std::sync::Arc::new(net),
        Some(std::sync::Arc::new(
            sts2_diff::honest::rollout::RolloutNet::v2(),
        )),
    ))
}

/// The run policy at `path`: `builtin` for the committed one, none without a path.
fn run_policy_of(
    path: Option<&str>,
) -> Result<Option<std::sync::Arc<sts2_diff::honest::policy::RunNet>>> {
    use sts2_diff::honest::policy::RunNet;
    let net = match path {
        None => return Ok(None),
        Some("builtin") => RunNet::builtin(),
        Some(path) => RunNet::load(path).map_err(Error::from)?,
    };
    Ok(Some(std::sync::Arc::new(net)))
}

/// `run-collect`: Ironclad runs on seeds `--first` .. `--first + --count` (train seeds) with the
/// run decisions by `--policy` (sampled from `--rng`, or `--greedy`) or, without one, the hand
/// rules, and the fights by the search. Writes the recorded decisions for
/// `python/train_run_ppo.py` under `--out`: `runs.tsv`, `dec.bin` (per decision run, first
/// candidate, candidates, chosen, floor as `i32`, log-probability as `f32`), `dec_tok.bin`,
/// `dec_scal.bin`, `cand_tok.bin`, `cand_scal.bin` and `cand_idx.bin`.
#[allow(
    clippy::print_stdout,
    reason = "the batch summary is the command's output"
)]
fn run_collect_cmd(args: &Args) -> Result<bool> {
    use std::fmt::Write as _;
    use sts2_diff::honest::{self, Bot, policy};
    let first: u64 = args.get("--first")?.parse()?;
    let count: u64 = args.get("--count")?.parse()?;
    let ascension: u8 = args.parse_or("--ascension", 0)?;
    let jobs: usize = args.parse_or("--jobs", 16)?;
    let rng: u64 = args.parse_or("--rng", 0)?;
    let out = args.get("--out")?;
    let net = args
        .get_opt("--policy")
        .map(policy::RunNet::load)
        .transpose()
        .map_err(Error::from)?;
    let collector = match &net {
        None => policy::Collector::Hand,
        Some(net) if args.has("--greedy") => policy::Collector::Greedy(net),
        Some(net) => policy::Collector::Sample(net),
    };
    let bot = Bot {
        controller: controller_of(args)?,
        net: None,
        run_policy: None,
    };
    // `--seeds FILE` replaces the range (diagnostics on chosen seeds).
    let seeds: Vec<u64> = match args.get_opt("--seeds") {
        Some(list) => std::fs::read_to_string(list)?
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?,
        None => (first..first + count).collect(),
    };
    std::fs::create_dir_all(out)?;
    let started = std::time::Instant::now();
    // A run the engine panics on is an error, not the end of the batch.
    // Each run's start and end go to `progress.tsv`, so a run that never ends is named.
    let progress = std::sync::Mutex::new(std::fs::File::create(format!("{out}/progress.tsv"))?);
    let note = |what: &str, seed: u64| {
        use std::io::Write as _;
        let _ = writeln!(progress.lock().expect("no poisoned lock"), "{what}\t{seed}");
    };
    let results = honest::on_threads(&seeds, jobs, |seed| {
        note("start", seed);
        let result = honest::caught(|| {
            honest::collect(&bot, collector, Character::Ironclad, seed, ascension, rng)
        });
        note("end", seed);
        result
    });
    let wall = started.elapsed().as_secs_f64();
    let mut files = policy::RecordFiles::default();
    let mut runs = String::new();
    let mut rows = Vec::new();
    let mut errors = 0;
    for (i, result) in results.into_iter().enumerate() {
        let (played, records) = match result {
            Ok(ok) => ok,
            Err(e) => {
                errors += 1;
                let _ = writeln!(runs, "{i}\t{}\terror\t0\t0\t0\t0\t{e}", seeds[i]);
                continue;
            }
        };
        let _ = writeln!(
            runs,
            "{i}\t{}\t{}\t{}\t{}\t{}\t{}",
            played.seed,
            played.end.name(),
            played.floor,
            u8::from(played.end == honest::End::Victory),
            records.len(),
            played.millis
        );
        files.push(u32::try_from(i).unwrap_or(u32::MAX), &records);
        rows.push(played);
    }
    files.write(out)?;
    std::fs::write(format!("{out}/runs.tsv"), runs)?;
    let summary = honest::summary(&rows, 0, errors, wall);
    print!("{summary}");
    std::fs::write(format!("{out}/summary.txt"), &summary)?;
    Ok(true)
}

/// Runs `command`; `Ok(false)` for a divergence or an anomaly.
fn run(command: &str, args: &Args) -> Result<bool> {
    match command {
        "gen" => gen_cmd(args).map(|()| true),
        "compare" => compare_cmd(args),
        "gen-batch" => gen_batch_cmd(args).map(|()| true),
        "compare-batch" => compare_batch_cmd(args),
        "run-gen" => run_gen_cmd(args).map(|()| true),
        "compare-run" => compare_run_cmd(args),
        "map" => map_cmd(args).map(|()| true),
        "compare-map" => compare_map_cmd(args),
        "history-predict" => history_predict_cmd(args).map(|()| true),
        "fixture" => fixture_cmd(args).map(|()| true),
        "run-fixture" => run_fixture_cmd(args).map(|()| true),
        "sweep" => sweep_cmd(args),
        "reach" => reach_cmd(args),
        "bot" => bot_cmd(args),
        "run-log" => run_log_cmd(args),
        "deck-probe" => deck_probe_cmd(args),
        "boss-starts" => boss_starts_cmd(args),
        "run-collect" => run_collect_cmd(args),
        "collect-starts" => collect_starts_cmd(args),
        "value-collect" => value_collect_cmd(args),
        "fights" => fights_cmd(args),
        "search-collect" => search_collect_cmd(args),
        "az-collect" => az_collect_cmd(args),
        "rollout-train" => rollout_train_cmd(args),
        "rollout-eval" => rollout_eval_cmd(args),
        "oracle-gaps" => Ok(oracle_gaps_cmd()),
        _ => Err(USAGE.into()),
    }
}

/// `capacities DIR`: the capacity survey's files in `DIR`, merged, as a table.
#[allow(
    clippy::print_stdout,
    reason = "the survey table is the command's output"
)]
fn capacities_cmd(raw: &[String]) -> Result<bool> {
    let dir = raw
        .first()
        .ok_or_else(|| format!("capacities needs a directory\n{USAGE}"))?;
    let (marks, files) = sts2_diff::capacity::merge_dir(dir)?;
    println!("{marks}");
    println!("({files} files)");
    Ok(true)
}

#[allow(clippy::print_stderr, reason = "the CLI reports errors on stderr")]
fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(2).collect();
    let command = std::env::args().nth(1).unwrap_or_default();
    sts2_diff::capacity::enable_from_env();
    let result = if command == "capacities" {
        capacities_cmd(&raw)
    } else {
        flags(&command)
            .ok_or_else(|| USAGE.into())
            .and_then(|(valued, presence)| {
                let args =
                    Args::parse(&raw, &valued, presence).map_err(|e| format!("{e}\n{USAGE}"))?;
                run(&command, &args)
            })
    };
    if let Err(e) = sts2_diff::capacity::finish() {
        eprintln!("sts2-diff: capacity survey: {e}");
    }
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("sts2-diff: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--real-game 1` turns on every real-game branch the referee has (`REAL_VARS` in
    /// `referee/referee.py`): without `real_calling_bell` Darv's `CallingBell` gave the referee
    /// `TestMode`'s fixed relics while the engine rolled them (PX15, `RL2S114`).
    #[test]
    fn real_game_actions_file_sets_every_real_branch() {
        let file = run_actions_file(Character::Necrobinder, 0, Act1::Overgrowth, true, &[]);
        for key in ["real_shop_prices", "real_cauldron", "real_calling_bell"] {
            assert_eq!(file[key], true, "{key}");
        }
        let file = run_actions_file(Character::Necrobinder, 0, Act1::Overgrowth, false, &[]);
        assert!(file.get("real_calling_bell").is_none());
    }
}
