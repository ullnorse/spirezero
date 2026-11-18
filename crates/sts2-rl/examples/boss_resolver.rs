//! RLN19: plays the act-2 boss benchmark's fights with the beam-search [`Greedy`] resolver.
//!
//! Reads a TSV on stdin (`id`, `seed`, prefix as comma-separated action indices; from
//! `python/rln19_act2.py resolver`), replays each prefix in an end-to-end run env (Necrobinder
//! A10) up to its last action, sets the env's resolver, takes that last action (the step into
//! the boss room), and prints `id<TAB>won<TAB>hp` after the fight: the resolver plays the whole
//! fight from the agent's own entry state.
//!
//! The first argument picks the resolver: `greedy` (the default), or RLN22's plan column alone
//! (`column`: the planner's choice at every decision, `column-argmax`: the best score).
//! `column-x2` plays as `column` but works out every combat decision's column once more and
//! counts the decisions on stderr: the callgrind difference between the two is the column's cost.
//! `STS2_THREADS` sets the env threads (default 4), `STS2_COLUMN_DEPTH` the column's search depth
//! (default [`FULL_DEPTH`]).
//!
//! [`Greedy`]: sts2_rl::resolve::Greedy

use std::hint::black_box;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sts2_core::Character;
use sts2_core::combat::Action;
use sts2_core::run::{Room, RoomType, Run, RunAction, RunDecision};
use sts2_rl::Resolver;
use sts2_rl::plan_column::{ColumnResolver, FULL_DEPTH, plan_column};
use sts2_rl::{CombatMode, HOLD, RUN_OBS_BYTES, RunConfig, RunEnvs, RunOptions, SeedSource};

/// [`ColumnResolver`], working out each combat decision's column once more and counting them.
#[derive(Debug)]
struct Twice {
    calls: Arc<AtomicU64>,
    depth: usize,
}

impl Resolver for Twice {
    fn reset(&mut self, _seed: u64) {}

    fn act(&mut self, run: &Run, legal: &[RunAction]) -> RunAction {
        if let RunDecision::Combat(combat) = run.pending_decision() {
            let big = matches!(
                run.state().room,
                Room::Combat {
                    room_type: RoomType::Elite | RoomType::Boss,
                    ..
                }
            );
            let actions: Vec<Action> = legal
                .iter()
                .filter_map(|a| match a {
                    RunAction::Combat(a) => Some(a.clone()),
                    _ => None,
                })
                .collect();
            black_box(plan_column(combat, big, &actions, self.depth));
            self.calls.fetch_add(1, Ordering::Relaxed);
        }
        ColumnResolver {
            argmax: false,
            depth: self.depth,
        }
        .act(run, legal)
    }
}

struct Fight {
    id: String,
    seed: u64,
    prefix: Vec<u16>,
}

/// The benchmark's fights, from stdin.
fn read_fights() -> Vec<Fight> {
    io::stdin()
        .lock()
        .lines()
        .map(|line| {
            let line = line.expect("stdin");
            let mut f = line.split('\t');
            let id = f.next().expect("id").to_owned();
            let seed = f.next().expect("seed").parse().expect("a seed");
            let prefix = f
                .next()
                .expect("prefix")
                .split(',')
                .map(|a| a.parse().expect("an action"))
                .collect();
            Fight { id, seed, prefix }
        })
        .collect()
}

fn main() {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "greedy".to_owned());
    let depth = std::env::var("STS2_COLUMN_DEPTH")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(FULL_DEPTH);
    let calls = Arc::new(AtomicU64::new(0));
    let resolver = || -> Box<dyn Resolver> {
        match mode.as_str() {
            "column-x2" => Box::new(Twice {
                calls: Arc::clone(&calls),
                depth,
            }),
            "greedy" => CombatMode::Greedy.resolver().expect("a resolver"),
            "column" => Box::new(ColumnResolver {
                argmax: false,
                depth,
            }),
            "column-argmax" => Box::new(ColumnResolver {
                argmax: true,
                depth,
            }),
            other => panic!("unknown resolver {other}"),
        }
    };
    let fights = read_fights();
    let config = RunConfig {
        character: Some(Character::Necrobinder),
        ascension: 10,
        combat: CombatMode::Agent,
        intents: true,
        options: RunOptions::E2E,
    };
    let seeds = SeedSource::List {
        seeds: fights.iter().map(|f| f.seed).collect(),
        cycle: false,
    };
    let n = fights.len();
    let mut envs = RunEnvs::new(n, seeds, config);
    envs.threads(
        std::env::var("STS2_THREADS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(4),
    );
    let longest = fights.iter().map(|f| f.prefix.len()).max().unwrap_or(0);
    // Each prefix's last action is taken at step `longest - 1`, with the resolver set.
    let start = |f: &Fight| longest - f.prefix.len();
    let mut actions = vec![HOLD; n];
    let mut won = vec![None::<(u8, i16)>; n];
    for t in 0..longest {
        for (i, f) in fights.iter().enumerate() {
            actions[i] = t.checked_sub(start(f)).map_or(HOLD, |k| f.prefix[k]);
            if t + 1 == longest {
                envs.set_resolver(i, Some(resolver()));
            }
        }
        envs.step(&actions)
            .expect("a prefix action the env refuses");
        if t + 1 < longest {
            assert!(
                envs.done().iter().all(|&d| d == 0),
                "a prefix ended its run"
            );
        }
    }
    let (events, finished, how, obs) = (envs.events(), envs.done(), envs.ends(), envs.obs());
    for (i, slot) in won.iter_mut().enumerate() {
        let row = &obs[i * RUN_OBS_BYTES..];
        let hp = i16::from_le_bytes([row[8], row[9]]);
        let w = u8::from(events[i].bosses > 0);
        *slot = Some((
            w,
            if finished[i] != 0 && how[i] != 2 {
                0
            } else {
                hp
            },
        ));
    }
    if mode == "column-x2" {
        writeln!(
            io::stderr(),
            "combat decisions {}",
            calls.load(Ordering::Relaxed)
        )
        .expect("stderr");
    }
    let mut out = io::stdout().lock();
    for (f, r) in fights.iter().zip(won) {
        let (w, hp) = r.expect("a result");
        writeln!(out, "{}\t{w}\t{hp}", f.id).expect("stdout");
    }
}
