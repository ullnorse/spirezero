//! `--mode run-env`: whole runs as RL episodes on [`sts2_rl::RunEnvs`] (`docs/specs/rl-2x-plan.md`,
//! stage 9).
//!
//! Each thread steps a [`RunEnvs`] of `--envs` envs over its seeds `t, t + threads, ...` of
//! `--seed-start..--seed-start + --runs` (a Necrobinder at ascension 0, or `--character`'s; `all`
//! plays seed `s` as the `s % 5`th character), timed from the batch's creation (so `Run::new`
//! is timed). Each step writes every live env's mask over the run space and its run
//! observation; the policy draws a uniform legal index from each mask with its own `SplitMix64`
//! seeded by the episode's seed. `--combat agent` (the default) plays the fights through the
//! combat space too; `greedy` and `random` settle them inside the env.
//!
//! A step is one agent decision; `actions` counts them, and the line also reports the engine
//! actions (with the automatic single options and the resolver's) and how the runs ended. The
//! digest is FNV over (seed, end, floor, decisions, engine actions, automatic, resolved, final
//! HP, max HP, gold, deck size) per run, summed, so it does not depend on `--envs` or
//! `--threads`.

use std::time::Instant;

use sts2_rl::seeds::SplitMix;
use sts2_rl::{RunConfig, RunEnd, RunEnvs, SeedSource};

use crate::{Chars, Config, Tally, fnv};

/// The run configuration of the bench's flags.
const fn run_config(config: &Config) -> RunConfig {
    RunConfig {
        character: match config.characters {
            Chars::Default => Some(sts2_core::Character::Necrobinder),
            Chars::One(c) => Some(c),
            Chars::All => None,
        },
        ascension: 0,
        combat: config.combat,
        intents: config.intents,
    }
}

/// A uniform legal index of `mask` (0 for an empty one: the env takes no action).
fn draw(mask: &[u64], policy: &mut SplitMix) -> u16 {
    let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
    if count == 0 {
        return 0;
    }
    let nth = u32::try_from(policy.below(count as usize)).expect("below count");
    let index = sts2_rl::action::nth_set(mask, nth).expect("nth is below the count of set bits");
    u16::try_from(index).expect("the run space has 355 indices")
}

/// The policy of env `i`'s current episode, if it has one.
fn policy(envs: &RunEnvs, i: usize) -> Option<SplitMix> {
    envs.episode(i)
        .map(|(_, seed)| SplitMix::new(fnv(&[seed, 0x52_554E])))
}

/// The run-env workload for the seeds `t, t + threads, ...`.
pub fn run_env(config: &Config, t: usize) -> Tally {
    let first = config.seed_start + u64::try_from(t).expect("fits u64");
    let seeds: Vec<u64> = (first..config.seed_start + config.runs)
        .step_by(config.threads)
        .collect();
    if seeds.is_empty() {
        return Tally::default();
    }
    let mut tally = Tally::default();
    let clock = Instant::now();
    let source = SeedSource::List {
        seeds,
        cycle: false,
    };
    let mut envs = RunEnvs::new(config.envs, source, run_config(config));
    let mut policies: Vec<Option<SplitMix>> = (0..envs.len()).map(|i| policy(&envs, i)).collect();
    let mut actions = vec![0; envs.len()];
    while envs.live() > 0 {
        for (i, (action, policy)) in actions.iter_mut().zip(&mut policies).enumerate() {
            if let Some(policy) = policy {
                *action = draw(envs.mask(i), policy);
            }
        }
        let digest = &mut tally.digest;
        let step = envs
            .step_with(&actions, |end| {
                let state = end.run.map(sts2_core::run::Run::state);
                let words = state.map_or([0; 4], |s| {
                    [
                        u64::from(s.hp.unsigned_abs()),
                        u64::from(s.max_hp.unsigned_abs()),
                        u64::from(s.gold.unsigned_abs()),
                        u64::try_from(s.deck.len()).expect("fits u64"),
                    ]
                });
                *digest = digest.wrapping_add(fnv(&[
                    end.seed,
                    end.end as u64,
                    u64::from(end.floor),
                    end.decisions,
                    end.actions,
                    end.auto,
                    end.resolved,
                    words[0],
                    words[1],
                    words[2],
                    words[3],
                ]));
            })
            .expect("the policy draws from the masks");
        tally.items += u64::from(step.ended);
        tally.wins += u64::from(step.won);
        tally.actions += step.decisions;
        tally.engine_actions += step.actions;
        tally.auto += step.auto;
        tally.resolved += step.resolved;
        tally.anomalies += u64::from(step.capped + step.stuck);
        tally.unmapped += step.unmapped;
        for (i, &done) in envs.done().iter().enumerate() {
            if done == 1 {
                if envs.ends()[i] == RunEnd::Unsupported as u8 {
                    tally.anomalies += 1;
                }
                tally.floors += u64::from(envs.floors()[i]);
                policies[i] = policy(&envs, i);
            }
        }
    }
    tally.timed = clock.elapsed();
    tally
}
