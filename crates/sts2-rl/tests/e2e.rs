//! The run env's end-to-end options (`docs/plans/necro-e2e.md`, WP1): action classes and their
//! counts, single-class auto-advance, the fight and decision caps, reward events, counters, and
//! that with the options off nothing changes. The classes read only what the player sees:
//! permuting the hidden draw order (and reseeding the RNG) leaves them as they were.

use sts2_core::Combat;
use sts2_core::rng::{Rng, RunRngSet};
use sts2_core::run::{RunAction, RunDecision};
use sts2_rl::action::{self, Mask};
use sts2_rl::classes::{DedupeScratch, combat_key, run_key};
use sts2_rl::run_action::{RUN_ACTIONS, RunMask, run_index_of};
use sts2_rl::seeds::SplitMix;
use sts2_rl::{CombatMode, RunConfig, RunEnd, RunEnvs, RunOptions, SeedSource};

/// A uniform legal index of `mask` (0 for an empty one).
fn draw(mask: &[u64], rng: &mut SplitMix) -> u16 {
    let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
    if count == 0 {
        return 0;
    }
    let nth = u32::try_from(rng.below(count as usize)).expect("below count");
    let index = action::nth_set(mask, nth).expect("nth is below the count");
    u16::try_from(index).expect("the space is small")
}

const fn range(start: u64, end: u64) -> SeedSource {
    SeedSource::Range {
        start,
        end,
        cycle: false,
    }
}

/// The plan's options at A10, with `combat` playing the fights.
fn e2e(combat: CombatMode) -> RunConfig {
    RunConfig {
        ascension: 10,
        combat,
        options: RunOptions::E2E,
        ..RunConfig::default()
    }
}

/// What random play over a batch saw.
#[derive(Debug, Default)]
struct Seen {
    episodes: u64,
    decisions: u64,
    merged: u64,
    auto_class: u64,
    unmapped_decisions: u64,
    saturated: [u64; 8],
    ends: [u64; 8],
    bosses: u64,
}

/// Checks env `i`'s class mask against its run: it is the plain mask less merged indices,
/// every kept index's count covers its class, and the counts are 0 off the mask.
fn check_classes(envs: &RunEnvs, i: usize) {
    let Some(run) = envs.run(i) else {
        return;
    };
    let legal = envs.legal(i);
    let mut plain = RunMask::new();
    plain.fill_run(run, legal);
    plain.retain_accepted(run, legal);
    let bits = envs.mask(i);
    let counts = envs.class_count(i);
    let mut total = 0_u32;
    for index in 0..RUN_ACTIONS {
        let set = bits[index / 64] >> (index % 64) & 1 == 1;
        if set {
            assert!(
                plain.contains(index),
                "env {i}: a class index the plain mask lacks"
            );
            assert!(counts[index] >= 1, "env {i}: a kept index without a count");
            total += u32::from(counts[index]);
        } else {
            assert_eq!(counts[index], 0, "env {i}: a count off the mask");
        }
    }
    assert!(
        total >= plain.count(),
        "env {i}: the classes cover {total} of {} mapped actions",
        plain.count()
    );
    // Every merged index has an equal-keyed kept index listed before it.
    let s = run.state();
    let decision = run.pending_decision();
    let combat = run.combat();
    let index = |a: &RunAction| match (combat, a) {
        (Some(c), RunAction::Combat(a)) => action::index_of(c, a),
        (_, a) => run_index_of(run, a),
    };
    for (p, a) in legal.iter().enumerate() {
        let Some(at) = index(a).filter(|&x| plain.position(x) == Some(p)) else {
            continue;
        };
        if bits[at / 64] >> (at % 64) & 1 == 1 {
            continue;
        }
        let key = run_key(s, &decision, a).expect("only keyed actions merge");
        let kept = legal[..p].iter().any(|b| {
            run_key(s, &decision, b).is_some_and(|k| k.same(&key, combat))
                && index(b).is_some_and(|x| bits[x / 64] >> (x % 64) & 1 == 1)
        });
        assert!(kept, "env {i}: {a:?} merged into no earlier kept member");
    }
}

/// Random play of `seeds` with `config` on `n` envs; `checked` checks the classes every step.
fn play(seeds: SeedSource, config: RunConfig, n: usize, checked: bool) -> Seen {
    let mut envs = RunEnvs::new(n, seeds, config);
    let policy = |envs: &RunEnvs, i: usize| envs.episode(i).map(|(_, seed)| SplitMix::new(seed));
    let mut policies: Vec<Option<SplitMix>> = (0..n).map(|i| policy(&envs, i)).collect();
    let mut actions = vec![0_u16; n];
    let mut seen = Seen::default();
    while envs.live() > 0 {
        for (i, (a, p)) in actions.iter_mut().zip(&mut policies).enumerate() {
            if let Some(p) = p {
                let count: u32 = envs.mask(i).iter().map(|w| w.count_ones()).sum();
                if config.options.dedupe && envs.run(i).is_some() {
                    assert!(
                        count >= 2,
                        "env {i}: a decision with {count} classes reached the agent"
                    );
                    if checked {
                        check_classes(&envs, i);
                    }
                }
                *a = draw(envs.mask(i), p);
            }
        }
        let mut bosses = 0;
        let step = envs
            .step_with(&actions, |end| bosses += u64::from(end.bosses))
            .expect("the policy draws from the masks");
        seen.bosses += bosses;
        seen.episodes += u64::from(step.ended);
        seen.decisions += step.decisions;
        seen.merged += step.merged;
        seen.auto_class += step.auto_class;
        seen.unmapped_decisions += step.unmapped_decisions;
        for (s, x) in seen.saturated.iter_mut().zip(step.saturated) {
            *s += x;
        }
        for (i, &done) in envs.done().iter().enumerate() {
            if done == 1 {
                seen.ends[usize::from(envs.ends()[i])] += 1;
                policies[i] = policy(&envs, i);
            }
        }
    }
    seen
}

#[test]
fn classes_merge_identical_actions_and_keep_counts() {
    let seen = play(
        range(23_000_000, 23_000_030),
        e2e(CombatMode::Agent),
        6,
        true,
    );
    assert_eq!(seen.episodes, 30);
    assert!(seen.merged > 0, "{seen:?}");
    assert!(seen.unmapped_decisions <= seen.decisions);
    assert!(seen.saturated[0] <= seen.decisions);
}

#[test]
fn a_thousand_smoke_runs_end_without_unsupported_or_stuck() {
    let seen = play(
        range(23_000_000, 23_001_000),
        e2e(CombatMode::Agent),
        64,
        false,
    );
    assert_eq!(seen.episodes, 1_000);
    let (unsupported, stuck) = (
        seen.ends[RunEnd::Unsupported as usize],
        seen.ends[RunEnd::Stuck as usize],
    );
    assert_eq!((unsupported, stuck), (0, 0), "{seen:?}");
    // End turn is a class of its own, so a decision collapses to one class only when every
    // option is the same (picking among identical cards): rare, but it happens.
    assert!(
        seen.auto_class > 0,
        "single-class decisions are taken: {seen:?}"
    );
}

#[test]
fn the_fight_cap_ends_a_run_as_a_capped_loss() {
    let config = RunConfig {
        options: RunOptions {
            turn_cap: 2,
            ..RunOptions::OFF
        },
        ..RunConfig::default()
    };
    let seen = play(range(0, 12), config, 4, false);
    let capped = seen.ends[RunEnd::FightCapped as usize];
    assert!(capped > 0, "{seen:?}");
    assert_eq!(seen.ends[RunEnd::Victory as usize], 0);
    assert_eq!(seen.ends.iter().sum::<u64>(), 12);
}

#[test]
fn the_decision_cap_ends_every_run_at_the_cap() {
    let config = RunConfig {
        options: RunOptions {
            decision_cap: 5,
            ..RunOptions::OFF
        },
        ..RunConfig::default()
    };
    let mut envs = RunEnvs::new(3, range(0, 6), config);
    let mut rng = SplitMix::new(1);
    let mut finished = Vec::new();
    while envs.live() > 0 {
        let actions: Vec<u16> = (0..3).map(|i| draw(envs.mask(i), &mut rng)).collect();
        envs.step_with(&actions, |end| finished.push((end.end, end.decisions)))
            .expect("legal");
    }
    assert_eq!(finished.len(), 6);
    for (end, decisions) in finished {
        // A random agent may die before five decisions; otherwise the cap ends the run.
        assert!(
            (end == RunEnd::DecisionCapped && decisions == 5)
                || (end == RunEnd::Died && decisions < 5),
            "{end:?} after {decisions}"
        );
    }
    assert!(
        RunEnd::DecisionCapped.is_cap() && RunEnd::FightCapped.is_cap() && !RunEnd::Died.is_cap()
    );
}

#[test]
fn events_report_floors_and_boss_kills() {
    let config = RunConfig {
        combat: CombatMode::Greedy,
        options: RunOptions::E2E,
        ..RunConfig::default()
    };
    let n = 8;
    let mut envs = RunEnvs::new(n, range(0, 40), config);
    let mut rng = SplitMix::new(3);
    // Per env, the floor at the episode's first decision plus the floors its steps entered,
    // and the bosses its steps killed.
    let floor_now = |envs: &RunEnvs, i: usize| {
        envs.run(i).map_or(0, |r| {
            u32::try_from(r.state().total_floor()).expect("small")
        })
    };
    let mut floors: Vec<u32> = (0..n).map(|i| floor_now(&envs, i)).collect();
    let mut bosses = vec![0_u32; n];
    let (mut kills, mut episodes) = (0, 0);
    while envs.live() > 0 {
        let actions: Vec<u16> = (0..n).map(|i| draw(envs.mask(i), &mut rng)).collect();
        let mut ended = Vec::new();
        envs.step_with(&actions, |end| {
            ended.push((end.env, end.floor, end.bosses, end.end));
        })
        .expect("legal");
        for (i, e) in envs.events().iter().enumerate() {
            floors[i] += u32::from(e.floors);
            bosses[i] += u32::from(e.bosses);
            assert_eq!(e.bosses > 0, e.boss_acts != 0, "env {i}: {e:?}");
        }
        for (i, floor, killed, end) in ended {
            assert_eq!(
                floors[i],
                u32::from(floor),
                "env {i}: the floor events add up"
            );
            assert_eq!(bosses[i], killed, "env {i}: the boss events add up");
            assert_eq!(envs.events()[i].won, u8::from(end == RunEnd::Victory));
            kills += killed;
            episodes += 1;
            floors[i] = floor_now(&envs, i);
            bosses[i] = 0;
        }
    }
    assert_eq!(episodes, 40);
    assert!(kills > 0, "greedy fights kill an act-1 boss in 40 runs");
}

#[test]
fn with_the_options_off_nothing_is_written() {
    let mut envs = RunEnvs::new(4, range(0, 4), RunConfig::default());
    let mut rng = SplitMix::new(5);
    for _ in 0..200 {
        let actions: Vec<u16> = (0..4).map(|i| draw(envs.mask(i), &mut rng)).collect();
        let step = envs.step(&actions).expect("legal");
        assert_eq!(
            (step.merged, step.auto_class, step.unmapped_decisions),
            (0, 0, 0)
        );
        assert_eq!(step.saturated, [0; 8]);
        assert!(envs.class_counts().iter().all(|&c| c == 0));
        assert!(
            envs.events()
                .iter()
                .all(|e| *e == sts2_rl::RunEvents::default())
        );
        if envs.live() == 0 {
            break;
        }
    }
}

/// The combat-space classes of `combat`: the deduped mask's bits and counts.
fn combat_classes(combat: &Combat) -> ([u64; action::MASK_WORDS], Vec<u8>) {
    let legal = combat.legal_actions();
    let mut mask = Mask::new();
    mask.fill(combat, &legal);
    let mut counts = vec![0; action::ACTIONS];
    mask.dedupe_with(
        &legal,
        |a| action::index_of(combat, a),
        |a| combat_key(combat, a),
        |a, b| a.same(b, Some(combat)),
        |_| true,
        &mut counts,
        &mut DedupeScratch::default(),
    );
    (*mask.bits(), counts)
}

#[test]
fn hidden_draw_order_does_not_change_the_classes() {
    let mut positions = 0;
    let mut merged_positions = 0;
    for seed in 0..40_u64 {
        let mut run = RunConfig::default().run(23_000_000 + seed).expect("a run");
        let mut rng = SplitMix::new(seed);
        for step in 0..600_u64 {
            if matches!(
                run.pending_decision(),
                RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
            ) {
                break;
            }
            if let Some(combat) = run.combat() {
                let base = combat_classes(combat);
                let mut other = combat.clone();
                other.edit(|s| {
                    s.player.draw.reverse();
                    let n = s.player.draw.len();
                    if n > 2 {
                        s.player
                            .draw
                            .rotate_left(usize::try_from(step).expect("small") % n);
                    }
                    s.rng = RunRngSet::new(&format!("CLASS{step}"));
                    for (i, c) in s.creatures.iter_mut().enumerate() {
                        if let Some(m) = &mut c.monster {
                            m.rng = Rng::new(step ^ u64::try_from(i).expect("small"));
                        }
                    }
                });
                assert_eq!(base, combat_classes(&other), "seed {seed} step {step}");
                positions += 1;
                merged_positions += u32::from(base.1.iter().any(|&c| c > 1));
            }
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            let _ = run.apply(legal[rng.below(legal.len())].clone());
        }
    }
    assert!(positions > 500, "{positions} combat positions");
    assert!(
        merged_positions > 100,
        "{merged_positions} positions with a merged class"
    );
}

#[test]
fn classes_do_not_depend_on_envs_or_threads() {
    let run = |n: usize, threads: usize| {
        let mut envs = RunEnvs::new(n, range(23_000_100, 23_000_112), e2e(CombatMode::Agent));
        envs.threads(threads);
        let policy = |envs: &RunEnvs, i: usize| envs.episode(i).map(|(_, s)| SplitMix::new(s));
        let mut policies: Vec<Option<SplitMix>> = (0..n).map(|i| policy(&envs, i)).collect();
        let mut out = Vec::new();
        while envs.live() > 0 {
            let actions: Vec<u16> = policies
                .iter_mut()
                .enumerate()
                .map(|(i, p)| p.as_mut().map_or(0, |p| draw(envs.mask(i), p)))
                .collect();
            envs.step_with(&actions, |end| {
                out.push((end.seed, end.end, end.decisions, end.floor, end.bosses));
            })
            .expect("legal");
            for (i, &done) in envs.done().iter().enumerate() {
                if done == 1 {
                    policies[i] = policy(&envs, i);
                }
            }
        }
        out.sort_unstable_by_key(|e| e.0);
        out
    };
    let base = run(1, 1);
    assert_eq!(run(5, 1), base);
    assert_eq!(run(12, 3), base);
}

/// A learner's resume: [`RunEnvs::restart`] on each env's seed, then [`HOLD`] replays of each
/// env's own actions, brings a fresh batch to the old one's state to the byte, and both go on
/// alike (the next seeds included).
#[test]
fn restart_and_hold_replay_a_batch() {
    use sts2_rl::HOLD;
    let n = 6;
    let config = RunConfig {
        options: RunOptions {
            priv_obs: true,
            ..RunOptions::E2E
        },
        ..e2e(CombatMode::Agent)
    };
    let mut a = RunEnvs::new(n, range(23_000_000, 23_001_000), config);
    let mut rng = SplitMix::new(7);
    // Each env's current seed and its run's actions so far, as a learner tracks them.
    let mut seeds: Vec<u64> = (0..n as u64).map(|i| 23_000_000 + i).collect();
    let mut next = n as u64;
    let mut history: Vec<Vec<u16>> = vec![Vec::new(); n];
    for _ in 0..300 {
        let actions: Vec<u16> = (0..n).map(|i| draw(a.mask(i), &mut rng)).collect();
        a.step(&actions).expect("legal");
        for i in 0..n {
            history[i].push(actions[i]);
            if a.done()[i] == 1 {
                seeds[i] = 23_000_000 + next;
                next += 1;
                history[i].clear();
            }
        }
    }
    let mut b = RunEnvs::new(n, range(23_000_000, 23_001_000), config);
    b.restart(&seeds, next);
    let longest = history.iter().map(Vec::len).max().unwrap_or(0);
    for k in 0..longest {
        let actions: Vec<u16> = history
            .iter()
            .map(|h| h.get(k).copied().unwrap_or(HOLD))
            .collect();
        b.step(&actions).expect("a replay is legal");
        assert!(b.done().iter().all(|&d| d == 0), "a replayed run ended");
    }
    assert_eq!(a.obs(), b.obs());
    assert_eq!(a.masks(), b.masks());
    assert_eq!(a.priv_obs(), b.priv_obs());
    for _ in 0..300 {
        let actions: Vec<u16> = (0..n).map(|i| draw(a.mask(i), &mut rng)).collect();
        a.step(&actions).expect("legal");
        b.step(&actions).expect("legal");
        assert_eq!(a.done(), b.done());
        assert_eq!(a.obs(), b.obs());
        assert_eq!(a.events(), b.events());
    }
    for i in 0..n {
        assert_eq!(a.episode(i).map(|e| e.1), b.episode(i).map(|e| e.1));
    }
}
