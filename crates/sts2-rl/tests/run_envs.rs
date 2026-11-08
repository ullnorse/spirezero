//! The run env (stage 9): the mask agrees with the run's legal list on every step, results do
//! not depend on the batch size or threads, single options are taken automatically, the hybrid
//! mode's resolver plays the fights, and the seed sources hand out what they say.

use sts2_core::run::RunDecision;
use sts2_rl::run_action::{RUN_ACTIONS, RUN_BASE, RunMask, check_run_mask};
use sts2_rl::seeds::SplitMix;
use sts2_rl::{CombatMode, RunConfig, RunEnd, RunEnvs, RunStepError, SeedSource};

/// FNV-1a over a few words.
fn fnv(words: &[u64]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325_u64;
    for w in words {
        for b in w.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }
    h
}

/// What a batch of random play did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Outcome {
    episodes: u64,
    wins: u64,
    decisions: u64,
    actions: u64,
    auto: u64,
    resolved: u64,
    refused: u64,
    max_floor: u16,
    digest: u64,
}

/// Checks env `i`'s mask buffer against its run's legal list: it is the list's mask (whose
/// every bit decodes to a listed action and which has a bit for every listed action the space
/// maps) less the indices the engine would refuse, and a run-level decision sets no combat bit.
fn check(envs: &RunEnvs, i: usize) {
    let Some(run) = envs.run(i) else {
        assert!(envs.mask(i).iter().all(|&w| w == 0));
        return;
    };
    let legal = envs.legal(i);
    let mut mask = RunMask::new();
    mask.fill_run(run, legal);
    mask.retain_accepted(run, legal);
    check_run_mask(run, legal, &mask).expect("the mask agrees with the list");
    let expected = *mask.bits();
    assert_eq!(
        envs.mask(i),
        &expected[..],
        "env {i}: the buffer is the legal list's mask"
    );
    if !matches!(run.pending_decision(), RunDecision::Combat(_)) {
        assert!(
            (0..RUN_BASE).all(|a| expected[a / 64] >> (a % 64) & 1 == 0),
            "env {i}: a run-level decision sets a combat bit"
        );
    }
}

/// A uniform legal index of `mask` (0 for an empty one).
fn draw(mask: &[u64], rng: &mut SplitMix) -> u16 {
    let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
    if count == 0 {
        return 0;
    }
    let nth = u32::try_from(rng.below(count as usize)).expect("below count");
    let index = sts2_rl::action::nth_set(mask, nth).expect("nth is below the count");
    u16::try_from(index).expect("the space is small")
}

/// Random play of `seeds` on `n` envs and `threads` threads; each episode's policy is seeded by
/// its seed, so trajectories do not depend on the env that plays them.
fn play(seeds: SeedSource, config: RunConfig, n: usize, threads: usize, checked: bool) -> Outcome {
    let mut envs = RunEnvs::new(n, seeds, config);
    envs.threads(threads);
    let policy = |envs: &RunEnvs, i: usize| envs.episode(i).map(|(_, seed)| SplitMix::new(seed));
    let mut policies: Vec<Option<SplitMix>> = (0..n).map(|i| policy(&envs, i)).collect();
    let mut actions = vec![0_u16; n];
    let mut outcome = Outcome::default();
    while envs.live() > 0 {
        for (i, (action, p)) in actions.iter_mut().zip(&mut policies).enumerate() {
            if checked {
                check(&envs, i);
            }
            if let Some(p) = p {
                *action = draw(envs.mask(i), p);
            }
        }
        let step = envs
            .step_with(&actions, |end| {
                outcome.digest = outcome.digest.wrapping_add(fnv(&[
                    end.seed,
                    end.end as u64,
                    u64::from(end.floor),
                    end.actions,
                    end.decisions,
                    end.auto,
                    end.resolved,
                ]));
                outcome.max_floor = outcome.max_floor.max(end.floor);
            })
            .expect("the policy draws from the masks");
        outcome.episodes += u64::from(step.ended);
        outcome.wins += u64::from(step.won);
        outcome.decisions += step.decisions;
        outcome.actions += step.actions;
        outcome.auto += step.auto;
        outcome.resolved += step.resolved;
        outcome.refused += step.refused;
        for (i, &done) in envs.done().iter().enumerate() {
            if done == 1 {
                policies[i] = policy(&envs, i);
            }
        }
    }
    outcome
}

const fn range(start: u64, end: u64) -> SeedSource {
    SeedSource::Range {
        start,
        end,
        cycle: false,
    }
}

#[test]
fn the_mask_agrees_with_the_legal_list_on_every_step() {
    let config = RunConfig {
        character: None,
        ..RunConfig::default()
    };
    let outcome = play(range(0, 10), config, 4, 1, true);
    assert_eq!(outcome.episodes, 10);
    assert!(outcome.decisions > 500, "{outcome:?}");
    assert!(
        outcome.auto > 0,
        "single options are taken automatically: {outcome:?}"
    );
    assert_eq!(outcome.resolved, 0);
}

#[test]
fn the_greedy_resolver_plays_the_fights() {
    let config = RunConfig {
        combat: CombatMode::Greedy,
        ..RunConfig::default()
    };
    let outcome = play(range(0, 3), config, 3, 1, true);
    assert_eq!(outcome.episodes, 3);
    assert!(outcome.resolved > 0, "{outcome:?}");
    // The agent only makes run-level choices.
    assert!(outcome.decisions < outcome.resolved, "{outcome:?}");
}

#[test]
fn results_do_not_depend_on_envs_or_threads() {
    let config = RunConfig::default();
    let base = play(range(100, 112), config, 1, 1, false);
    assert_eq!(play(range(100, 112), config, 5, 1, false), base);
    assert_eq!(play(range(100, 112), config, 12, 3, false), base);
    let random = RunConfig {
        combat: CombatMode::Random,
        ..config
    };
    let base = play(range(0, 6), random, 1, 1, false);
    assert_eq!(play(range(0, 6), random, 4, 2, false), base);
}

#[test]
fn an_illegal_action_is_refused_and_nothing_moves() {
    let mut envs = RunEnvs::new(1, range(7, 8), RunConfig::default());
    let obs = envs.obs().to_vec();
    let mask = envs.mask(0).to_vec();
    let illegal = (0..RUN_ACTIONS)
        .find(|&a| mask[a / 64] >> (a % 64) & 1 == 0)
        .expect("an illegal index");
    let illegal = u16::try_from(illegal).expect("small");
    assert_eq!(
        envs.step(&[illegal]),
        Err(RunStepError::Illegal {
            env: 0,
            action: illegal
        })
    );
    assert_eq!(
        envs.step(&[]),
        Err(RunStepError::Length {
            expected: 1,
            got: 0
        })
    );
    assert_eq!(envs.obs(), &obs[..]);
}

#[test]
fn seed_sources_hand_out_their_seeds() {
    let list = SeedSource::List {
        seeds: vec![5, 9],
        cycle: false,
    };
    assert_eq!(
        (0..3).map(|k| list.seed(k)).collect::<Vec<_>>(),
        [Some(5), Some(9), None]
    );
    let cycle = SeedSource::List {
        seeds: vec![5, 9],
        cycle: true,
    };
    assert_eq!(cycle.seed(3), Some(9));
    assert_eq!(range(3, 5).seed(1), Some(4));
    assert_eq!(range(3, 5).seed(2), None);
    assert!(range(3, 3).is_empty());
    let random = SeedSource::Random { master: 1 };
    assert_ne!(random.seed(0), random.seed(1));
    assert_eq!(random.seed(7), SeedSource::Random { master: 1 }.seed(7));
    // A finite source leaves the envs it cannot feed idle.
    let mut envs = RunEnvs::new(3, range(0, 2), RunConfig::default());
    assert_eq!(envs.live(), 2);
    assert!(envs.mask(2).iter().all(|&w| w == 0));
    assert_eq!(envs.episode(1), Some((1, 1)));
    let _ = envs.step(&[0, 0, 0]);
}

#[test]
fn the_first_decision_is_neow_and_the_run_ends_are_reported() {
    let envs = RunEnvs::new(1, range(0, 1), RunConfig::default());
    let run = envs.run(0).expect("a run");
    assert!(matches!(run.pending_decision(), RunDecision::Ancient(_)));
    assert_eq!(RunEnd::Died as u8, 1);
}

#[test]
fn the_final_observation_keeps_the_ended_episode() {
    // A finished episode is reset before the step returns; its observation must still be
    // readable for terminal HP/floor shaping.
    let config = RunConfig {
        ascension: 10,
        combat: CombatMode::Random,
        ..RunConfig::default()
    };
    let mut envs = RunEnvs::new(1, range(9_000_000, 9_000_001), config);
    let mut actions = [0_u16; 1];
    while envs.live() > 0 {
        let mut rng = SplitMix::new(9_000_000);
        actions[0] = draw(envs.mask(0), &mut rng);
        let step = envs.step(&actions).expect("a legal action");
        if step.ended > 0 {
            assert!(
                envs.final_obs().iter().any(|&b| b != 0),
                "the ended episode's observation is kept"
            );
            assert_ne!(envs.final_obs(), envs.obs(), "the reset wrote a new one");
            return;
        }
    }
    panic!("the seed ended without an episode");
}
