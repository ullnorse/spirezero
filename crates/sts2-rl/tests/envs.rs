//! The `Envs` batch against the RL bench's workload: the mask agrees with the legal list on
//! every step, the caches agree with the slow paths (their oracles are debug assertions, on
//! in tests), the results do not depend on the env count or the threads, and snapshots and
//! the start corpus round-trip.

use sts2_core::combat::Encounter;
use sts2_core::digest::canonical_digest;
use sts2_core::run::rooms::OVERGROWTH;
use sts2_core::{Character, Combat, CombatConfig, Status};
use sts2_rl::action::nth_set;
use sts2_rl::{Envs, Order, Start, StartRecord, StartSource, check_mask};

/// The RL bench's start for `seed` (`sts2-bench`'s `fight_start`): W1-all, character
/// `seed % 5` on Overgrowth encounter `seed % 22`.
fn bench_start(seed: u64) -> Start {
    let character = Character::ALL[usize::try_from(seed % 5).expect("below 5")];
    let info = &OVERGROWTH.encounters
        [usize::try_from(seed).expect("fits usize") % OVERGROWTH.encounters.len()];
    let encounter = Encounter::from_entry(info.id).expect("every Overgrowth encounter is ported");
    let config = CombatConfig::new(character, &format!("W1S{seed}"), 0)
        .expect("every starter card is ported")
        .with_encounter(encounter);
    Start {
        seed,
        combat: Combat::new(config),
    }
}

/// The bench's policy RNG (`SplitMix64`).
struct Policy(u64);

impl Policy {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The bench's FNV-1a over words.
fn fnv(words: &[u64]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325_u64;
    for w in words {
        for b in w.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }
    h
}

/// What a run of [`play`] produced.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    episodes: u64,
    wins: u64,
    actions: u64,
    digest: u64,
    unmapped: u64,
}

/// Plays `seeds` × `reps` episodes on `envs` envs and `threads` threads the way the RL bench
/// does, checking the mask against the legal list before every step.
fn play(seeds: std::ops::Range<u64>, reps: u64, envs: usize, threads: usize) -> Outcome {
    let starts = seeds.map(bench_start).collect();
    let source = StartSource::new(starts, Order::Sequential { reps });
    let mut envs = Envs::new(envs, source, 0);
    envs.threads(threads);
    let policy = |envs: &Envs, i: usize| {
        envs.episode(i).map(|e| {
            let seed = envs.source().starts()[e.start].seed;
            Policy(fnv(&[seed, e.rep]))
        })
    };
    let mut policies: Vec<_> = (0..envs.len()).map(|i| policy(&envs, i)).collect();
    let mut actions = vec![0; envs.len()];
    let mut legal = Vec::new();
    let mut outcome = Outcome {
        episodes: 0,
        wins: 0,
        actions: 0,
        digest: 0,
        unmapped: 0,
    };
    while envs.live() > 0 {
        for (i, policy) in policies.iter_mut().enumerate() {
            let combat = envs.combat(i);
            let Some(policy) = policy else { continue };
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                continue;
            }
            combat.legal_actions_into(&mut legal);
            let mut mask = sts2_rl::Mask::new();
            outcome.unmapped += u64::from(mask.fill(combat, &legal));
            assert_eq!(
                mask.bits()[..],
                envs.mask(i)[..],
                "env {i}'s mask is its fight's"
            );
            check_mask(combat, &legal, &mask).expect("the mask agrees with the list");
            let count = mask.count();
            if count == 0 {
                continue;
            }
            let nth = u32::try_from(policy.next() % u64::from(count)).expect("below count");
            actions[i] = u16::try_from(nth_set(envs.mask(i), nth).expect("below count"))
                .expect("140 indices");
        }
        let step = envs
            .step_with(&actions, |end| {
                outcome.digest = outcome.digest.wrapping_add(fnv(&[
                    end.seed,
                    end.episode.rep,
                    u64::from(end.won()),
                    end.actions,
                    canonical_digest(end.combat.state()),
                ]));
            })
            .expect("the policy draws from the masks");
        outcome.episodes += u64::from(step.ended);
        outcome.wins += u64::from(step.won);
        outcome.actions += step.actions;
        for (i, &done) in envs.done().iter().enumerate() {
            if done == 1 {
                policies[i] = policy(&envs, i);
            }
        }
    }
    outcome
}

#[test]
fn mask_and_caches_agree_on_a_slice_of_the_rl_bench() {
    // Every character and encounter at least once; the oracles run on every observation.
    let outcome = play(0..110, 2, 16, 1);
    assert_eq!(outcome.episodes, 220);
    assert_eq!(outcome.unmapped, 0);
    assert!(outcome.actions > 3_000, "{outcome:?}");
}

#[test]
fn results_do_not_depend_on_envs_or_threads() {
    let base = play(0..30, 3, 1, 1);
    assert_eq!(play(0..30, 3, 7, 1), base);
    assert_eq!(play(0..30, 3, 64, 3), base);
}

/// The whole RL bench (2,200 seeds × 50) with the mask checked on every step, and its digest.
/// Slow in a debug build: `cargo test --profile regress -p sts2-rl -- --ignored`.
#[test]
#[ignore = "the full RL bench; run in the regress profile"]
fn mask_agrees_on_every_step_of_the_rl_bench() {
    let outcome = play(0..2200, 50, 64, 1);
    assert_eq!(outcome.unmapped, 0);
    assert_eq!(outcome.episodes, 110_000);
    assert_eq!(outcome.actions, 2_548_971);
    assert_eq!(format!("{:016x}", outcome.digest), "372d441d8132f8de");
}

#[test]
fn restore_puts_back_the_position_and_its_buffers() {
    let source = StartSource::new(vec![bench_start(3)], Order::Cycle);
    let mut envs = Envs::new(2, source, 0);
    let snapshot = envs.snapshot(0);
    let (obs, mask) = (envs.obs().to_vec(), envs.masks().to_vec());
    let end_turn = u16::try_from(sts2_rl::action::END_TURN).expect("fits");
    envs.step(&[end_turn, end_turn]).expect("end turn is legal");
    assert_ne!(envs.obs(), &obs[..]);
    envs.restore(0, &snapshot);
    envs.restore(1, &snapshot);
    assert_eq!(envs.obs(), &obs[..]);
    assert_eq!(envs.masks(), &mask[..]);
}

#[test]
fn an_illegal_action_is_refused_and_nothing_moves() {
    let source = StartSource::new(vec![bench_start(5)], Order::Cycle);
    let mut envs = Envs::new(1, source, 0);
    let obs = envs.obs().to_vec();
    let done = u16::try_from(sts2_rl::action::DONE).expect("fits");
    assert_eq!(
        envs.step(&[done]),
        Err(sts2_rl::StepError::Illegal {
            env: 0,
            action: done
        })
    );
    assert_eq!(
        envs.step(&[]),
        Err(sts2_rl::StepError::Length {
            expected: 1,
            got: 0
        })
    );
    assert_eq!(envs.obs(), &obs[..]);
}

#[test]
fn a_start_record_round_trips_and_builds_its_combat() {
    let line = r#"{"character":"Silent","seed":"SW3/Silent/A4/underdocks/F6","ascension":4,"act":0,"encounter":"SLIMES_WEAK","deck":[["STRIKE_SILENT",1,null,0],["DEFEND_SILENT",0,null,0],["NEUTRALIZE",0,null,0],["SURVIVOR",0,null,0]],"relics":["RING_OF_THE_SNAKE"],"potion_slots":2,"potions":[null,"FIRE_POTION"],"hp":51,"max_hp":70,"max_energy":3,"gold":120,"floor":6,"map_coord":[2,5]}"#;
    let record = StartRecord::from_json(line).expect("a record");
    assert_eq!(record.character, Character::Silent);
    assert_eq!(record.deck[0].upgrade_level, 1);
    assert_eq!(
        StartRecord::from_json(&record.to_json()),
        Ok(record.clone())
    );
    let combat = record.combat().expect("a playable start");
    let s = combat.state();
    assert_eq!(s.creature(s.player.creature).hp, 51);
    assert_eq!(s.player.gold, 120);
    assert_eq!(s.ascension, 4);
    let source = StartSource::parse(&format!("{line}\n\n{line}\n"), Order::Random).expect("two");
    assert_eq!(source.starts().len(), 2);
    assert!(StartRecord::from_json(&line.replace("SLIMES_WEAK", "NOPE")).is_err());
}
