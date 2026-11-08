//! `--mode rl`: the workload a vectorised RL trainer (`PufferLib`) drives
//! (`docs/specs/rl-2x-plan.md`, section 3, "RL bench mode"), on [`sts2_rl::Envs`].
//!
//! Each thread steps an [`Envs`] of `--envs` fights, one action per env per step. An episode is
//! a fight from a start state: seed `s` is W1-all's start for character `s % 5` (or
//! `--character`'s only) on Overgrowth encounter `s % 22`, built once and untimed; the thread's
//! starts are its seeds `t, t + threads, ...`, each played `--reps` times in turn
//! ([`Order::Sequential`]), and an env resets from a start with `clone_from` (timed). A step
//! writes every live env's legal-action bitset (the fixed 140-entry space, section 7.2) and
//! observation (section 7.1); the policy draws a uniform legal index from each env's bitset with
//! its own `SplitMix64` (seeded by the start's seed and the repetition, never an engine stream),
//! and `Envs::step` decodes and applies them. The static part of the observation (character,
//! encounter, deck, relics, potions) is written at each reset.
//!
//! The digest is FNV over (seed, repetition, won, actions, the final state's canonical digest)
//! per episode, summed, so it does not depend on `--envs` or `--threads` (the episode an env
//! plays does not change its trajectory). Variants for cost attribution: `--no-intents` (the
//! observation without intents), `--no-obs` (mask only) and `--no-mask` (no `Envs`: the policy
//! draws from the legal list as W1 does, the engine alone; the trajectories, and so the digest,
//! differ from the masked modes'). `--check-mask` also checks on every step that the mask and
//! the legal list agree ([`sts2_rl::check_mask`]). `--starts FILE` plays a start corpus
//! (`cargo xtask sweep --emit-starts FILE`, JSON lines) instead of W1-all's starts: seed `i`
//! is the corpus's `i`th playable record, and `--seeds` is ignored.

use std::time::Instant;

use sts2_core::digest::canonical_digest;
use sts2_core::{Character, Combat, Status};
use sts2_rl::start::Order;
use sts2_rl::{Envs, Start, StartSource, check_mask};

use crate::{Chars, Config, MAX_FIGHT_ACTIONS, Policy, Tally, fight_start, fnv};

/// The dynamic observation's size, for the report.
pub const OBS_BYTES: usize = sts2_rl::OBS_BYTES;

/// The character of RL start `seed`.
fn character(config: &Config, seed: u64) -> Character {
    match config.characters {
        Chars::One(c) => c,
        Chars::Default | Chars::All => Character::ALL[usize::try_from(seed % 5).expect("below 5")],
    }
}

/// The thread's starts: seeds `t, t + threads, ...` of the range, or of the corpus
/// (`--starts`, seed `i` is its `i`th playable record).
fn starts(config: &Config, t: usize) -> Vec<Start> {
    if let Some(path) = &config.starts {
        let corpus = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let source =
            StartSource::parse(&corpus, Order::Cycle).unwrap_or_else(|e| panic!("{path}: {e}"));
        return source
            .starts()
            .iter()
            .skip(t)
            .step_by(config.threads)
            .cloned()
            .collect();
    }
    let first = config.seed_start + u64::try_from(t).expect("fits u64");
    (first..config.seed_start + config.seeds)
        .step_by(config.threads)
        .map(|seed| Start {
            seed,
            combat: fight_start(character(config, seed), seed),
        })
        .collect()
}

/// One episode's contribution to the digest.
fn episode_digest(seed: u64, rep: u64, won: bool, actions: u64, combat: &Combat) -> u64 {
    fnv(&[
        seed,
        rep,
        u64::from(won),
        actions,
        canonical_digest(combat.state()),
    ])
}

/// The policy of env `i`'s current episode, if it has one.
fn policy(envs: &Envs, i: usize) -> Option<Policy> {
    let episode = envs.episode(i)?;
    let seed = envs.source().starts()[episode.start].seed;
    Some(Policy(fnv(&[seed, episode.rep])))
}

/// A uniform legal index of `mask` (0 for an empty mask: `Envs` ends that episode without an
/// action).
fn draw(mask: &[u64], policy: &mut Policy) -> u16 {
    let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
    if count == 0 {
        return 0;
    }
    let nth = u32::try_from(policy.pick(count as usize)).expect("below count");
    let index = sts2_rl::action::nth_set(mask, nth).expect("nth is below the count of set bits");
    u16::try_from(index).expect("the space has 140 indices")
}

/// The RL workload for the seeds `t, t + threads, ...`.
pub fn rl(config: &Config, t: usize) -> Tally {
    let starts = starts(config, t);
    if starts.is_empty() {
        return Tally::default();
    }
    if !config.mask {
        return engine_only(config, &starts);
    }
    let mut tally = Tally::default();
    let source = StartSource::new(starts, Order::Sequential { reps: config.reps });
    let clock = Instant::now();
    let mut envs = Envs::with_observation(config.envs, source, 0, config.obs, config.intents);
    let mut policies: Vec<Option<Policy>> = (0..envs.len()).map(|i| policy(&envs, i)).collect();
    let mut actions = vec![0; envs.len()];
    let mut legal = Vec::new();
    while envs.live() > 0 {
        for (i, (action, policy)) in actions.iter_mut().zip(&mut policies).enumerate() {
            if let Some(policy) = policy
                && matches!(
                    envs.combat(i).status(),
                    Status::PlayerTurn | Status::Decision
                )
            {
                let mask = envs.mask(i);
                if config.check_mask {
                    envs.combat(i).legal_actions_into(&mut legal);
                    let mut filled = sts2_rl::Mask::new();
                    filled.fill(envs.combat(i), &legal);
                    assert_eq!(filled.bits()[..], mask[..], "the env's mask is its fight's");
                    check_mask(envs.combat(i), &legal, &filled)
                        .expect("the mask agrees with the list");
                }
                *action = draw(mask, policy);
            }
        }
        let digest = &mut tally.digest;
        let step = envs
            .step_with(&actions, |end| {
                *digest = digest.wrapping_add(episode_digest(
                    end.seed,
                    end.episode.rep,
                    end.won(),
                    end.actions,
                    end.combat,
                ));
            })
            .expect("the policy draws from the masks");
        tally.items += u64::from(step.ended);
        tally.wins += u64::from(step.won);
        tally.actions += step.actions;
        tally.anomalies += u64::from(step.capped);
        tally.unmapped += step.unmapped;
        for (i, &done) in envs.done().iter().enumerate() {
            if done == 1 {
                policies[i] = policy(&envs, i);
            }
        }
    }
    tally.timed = clock.elapsed();
    tally
}

/// `--no-mask`: the same episodes with the policy drawing from the legal list, no extraction.
fn engine_only(config: &Config, starts: &[Start]) -> Tally {
    let mut tally = Tally::default();
    let mut legal = Vec::new();
    let clock = Instant::now();
    let mut combat = starts[0].combat.clone();
    for start in starts {
        for rep in 0..config.reps {
            combat.clone_from(&start.combat);
            let mut policy = Policy(fnv(&[start.seed, rep]));
            let mut actions = 0;
            while matches!(combat.status(), Status::PlayerTurn | Status::Decision)
                && actions < MAX_FIGHT_ACTIONS
            {
                combat.legal_actions_into(&mut legal);
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
                .wrapping_add(episode_digest(start.seed, rep, won, actions, &combat));
        }
    }
    tally.timed = clock.elapsed();
    tally
}
