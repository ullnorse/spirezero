//! Practice starts (`sts2_rl::practice`): a start is the captured fight on new streams, legal at
//! its first decision; it replays exactly given its reseed; in the batch, practice episodes end
//! at their first decision outside combat, take no serial of the seed source and do not depend
//! on the threads; and capturing alone changes no trajectory.

use std::collections::HashSet;

use sts2_core::run::{Room, RoomType, Run, RunAction, RunDecision};
use sts2_rl::practice::{self, CELL_NAMES, Cell, SegmentEnd, Snapshot};
use sts2_rl::run_obs::RUN_OBS_BYTES;
use sts2_rl::seeds::SplitMix;
use sts2_rl::{
    CombatMode, PRACTICE_EPISODE, PracticeConfig, RunConfig, RunEnd, RunEnvs, RunObserver,
    RunOptions, SeedSource,
};

const ELITE1: Cell = Cell {
    boss: false,
    act: 0,
};
const BOSS1: Cell = Cell { boss: true, act: 0 };

/// FNV-1a over bytes.
fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(h, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3))
}

fn config(combat: CombatMode) -> RunConfig {
    RunConfig {
        combat,
        options: RunOptions {
            events: true,
            priv_obs: true,
            ..RunOptions::OFF
        },
        ..RunConfig::default()
    }
}

fn practice(slots: usize) -> PracticeConfig {
    PracticeConfig {
        slots,
        pool: 64,
        max_uses: 0,
        min_active: 1,
        seed: 11,
        ..PracticeConfig::default()
    }
}

/// A uniform legal index of `mask`.
fn draw(mask: &[u64], rng: &mut SplitMix) -> u16 {
    let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
    if count == 0 {
        return 0;
    }
    let nth = u32::try_from(rng.below(count as usize)).expect("below count");
    let index = sts2_rl::action::nth_set(mask, nth).expect("nth is below the count");
    u16::try_from(index).expect("the space is small")
}

/// One random step of every env.
fn step(envs: &mut RunEnvs, rng: &mut SplitMix) {
    let actions: Vec<u16> = (0..envs.len()).map(|i| draw(envs.mask(i), rng)).collect();
    envs.step(&actions).expect("drawn from the masks");
}

/// Greedy fights and random run choices until the pool holds a snapshot of every cell of
/// `cells`.
fn snapshots(cells: &[Cell]) -> RunEnvs {
    let mut envs = RunEnvs::new(
        16,
        SeedSource::Range {
            start: 500,
            end: 100_000,
            cycle: false,
        },
        config(CombatMode::Greedy),
    );
    envs.set_practice(Some(practice(0)));
    let mut rng = SplitMix::new(3);
    for _ in 0..20_000 {
        let pool = envs.practice().expect("set");
        if cells.iter().all(|&c| pool.sizes()[c.index()] > 0) {
            return envs;
        }
        step(&mut envs, &mut rng);
    }
    panic!("no snapshot of every cell of {cells:?}");
}

fn first(envs: &RunEnvs, cell: Cell) -> Snapshot {
    envs.practice()
        .expect("set")
        .snapshots(cell)
        .next()
        .expect("a snapshot")
        .clone()
}

fn observe(run: &Run) -> Vec<u8> {
    let mut out = vec![0; RUN_OBS_BYTES];
    RunObserver::new(true).observe(run, &mut out);
    out
}

#[test]
fn a_start_is_the_cells_fight_on_new_streams_and_legal() {
    let envs = snapshots(&[ELITE1, BOSS1]);
    for cell in [ELITE1, BOSS1] {
        let snap = first(&envs, cell);
        assert_eq!(snap.cell, cell);
        let mut plain = snap.run.clone();
        plain.apply(snap.choice.clone()).expect("the captured move");
        let started = practice::start(&snap, 1).expect("a start");
        let combat = started.combat().expect("in the fight");
        let want = if cell.boss {
            RoomType::Boss
        } else {
            RoomType::Elite
        };
        let (
            Room::Combat {
                room_type,
                encounter,
            },
            Room::Combat {
                encounter: plain_encounter,
                ..
            },
        ) = (&started.state().room, &plain.state().room)
        else {
            panic!("{}: not a fight room", CELL_NAMES[cell.index()]);
        };
        assert_eq!(*room_type, want);
        assert_eq!(encounter, plain_encounter, "the rooms were rolled up front");
        assert_eq!(combat.state().player.turn_number, 1);
        assert_eq!(started.state().deck, plain.state().deck);
        assert_eq!(started.state().total_floor(), plain.state().total_floor());
        assert_ne!(started.rng().seed(), plain.rng().seed(), "new streams");
        // Legal: the engine accepts at least one listed action, and every one it lists for a
        // first decision of a fight is a combat action.
        let legal = started.legal_actions();
        assert!(legal.iter().any(|a| started.accepts(a)));
        // The fight's randomness differs between reseeds (hands, intents).
        let seen: HashSet<Vec<u8>> = (1..=8)
            .map(|r| observe(&practice::start(&snap, r).expect("a start")))
            .collect();
        assert!(
            seen.len() >= 2,
            "{}: reseeds look alike",
            CELL_NAMES[cell.index()]
        );
    }
}

#[test]
fn a_start_replays_exactly_given_its_reseed() {
    let envs = snapshots(&[ELITE1]);
    let snap = first(&envs, ELITE1);
    let mut a = practice::start(&snap, 77).expect("a start");
    let mut b = practice::start(&snap, 77).expect("a start");
    assert_eq!(a, b);
    let mut rng = SplitMix::new(5);
    for _ in 0..150 {
        let legal: Vec<_> = a
            .legal_actions()
            .into_iter()
            .filter(|x| a.accepts(x))
            .collect();
        if legal.is_empty() {
            break;
        }
        let action = legal[rng.below(legal.len())].clone();
        a.apply(action.clone()).expect("accepted");
        b.apply(action).expect("accepted");
        assert_eq!(a, b);
    }
    assert_ne!(
        practice::start(&snap, 78).expect("a start"),
        practice::start(&snap, 77).expect("a start")
    );
}

/// What `steps` random steps of a batch with practice on half the envs did.
#[derive(Debug, Default, PartialEq, Eq)]
struct Trace {
    digest: u64,
    practice_ends: [u64; 9],
    run_serials: Vec<u64>,
    cells_started: u64,
}

fn trace(threads: usize, combat: CombatMode, steps: usize) -> Trace {
    let n = 4;
    let mut envs = RunEnvs::new(
        n,
        SeedSource::Range {
            start: 900,
            end: 100_000,
            cycle: false,
        },
        config(combat),
    );
    envs.threads(threads);
    envs.set_practice(Some(practice(n / 2)));
    let mut rng = SplitMix::new(9);
    let mut out = Trace::default();
    for _ in 0..steps {
        let actions: Vec<u16> = (0..n).map(|i| draw(envs.mask(i), &mut rng)).collect();
        let mut ended = Vec::new();
        envs.step_with(&actions, |end| {
            ended.push((
                end.episode,
                end.end,
                end.cell,
                end.run.map(|r| r.combat().is_none()),
            ));
        })
        .expect("drawn from the masks");
        for (episode, end, cell, out_of_combat) in ended {
            if let Some(cell) = cell {
                assert_eq!(episode, PRACTICE_EPISODE);
                out.practice_ends[end as usize] += 1;
                if end == RunEnd::PracticeEnd {
                    assert_eq!(out_of_combat, Some(true), "{cell:?} ended outside combat");
                }
            } else {
                out.run_serials.push(episode);
            }
        }
        for i in 0..n {
            if envs.done()[i] == 1 && envs.ends()[i] == RunEnd::PracticeEnd as u8 {
                assert!(envs.final_masks()[i * 6..][..6].iter().any(|&w| w != 0));
                assert!(envs.final_priv().iter().any(|&b| b != 0));
            }
        }
        out.digest = fnv(out.digest, envs.obs());
        out.digest = fnv(out.digest, envs.cells());
        out.digest = fnv(out.digest, envs.ended_cells());
        out.digest = fnv(out.digest, envs.final_obs());
        out.digest = fnv(out.digest, envs.ends());
    }
    // The episodes still playing: practice ones have no serial.
    for i in 0..n {
        match (envs.cells()[i], envs.episode(i)) {
            (0, Some((serial, _))) => out.run_serials.push(serial),
            (_, Some((serial, _))) => assert_eq!(serial, PRACTICE_EPISODE),
            _ => {}
        }
    }
    let s = envs.practice().expect("set").stats();
    out.cells_started = s.started.iter().sum();
    out
}

#[test]
fn practice_episodes_end_outside_combat_and_do_not_depend_on_threads() {
    let base = trace(1, CombatMode::Greedy, 150);
    assert!(base.cells_started > 0, "{base:?}");
    assert!(
        base.practice_ends[RunEnd::PracticeEnd as usize] > 0,
        "{base:?}"
    );
    // The normal runs (ended or playing) take the source's serials in order, none skipped for
    // practice.
    let mut serials = base.run_serials.clone();
    serials.sort_unstable();
    assert_eq!(serials, (0..serials.len() as u64).collect::<Vec<_>>());
    assert_eq!(trace(4, CombatMode::Greedy, 150), base);
}

#[test]
fn capturing_alone_changes_no_trajectory() {
    let play = |capture: Option<PracticeConfig>| {
        let mut envs = RunEnvs::new(
            4,
            SeedSource::Range {
                start: 40,
                end: 100_000,
                cycle: false,
            },
            config(CombatMode::Greedy),
        );
        envs.set_practice(capture);
        let mut rng = SplitMix::new(1);
        let mut digest = 0;
        for _ in 0..150 {
            step(&mut envs, &mut rng);
            digest = fnv(digest, envs.obs());
            digest = fnv(digest, envs.ends());
            digest = fnv(digest, envs.cells());
        }
        (
            digest,
            envs.practice()
                .map(|p| p.stats().captured.iter().sum::<u64>()),
        )
    };
    let (off, none) = play(None);
    let (on, captured) = play(Some(practice(0)));
    assert_eq!(none, None);
    assert!(captured.is_some_and(|c| c > 0));
    assert_eq!(on, off);
}

const BOSS3: Cell = Cell { boss: true, act: 2 };

/// A `BOSS_ACT3` snapshot of A10 Necrobinder run `RUN<seed>`: act 1's ancient taken, the run moved
/// to act 3 (`Run::debug_enter_act`), greedy fights and the first accepted option elsewhere at
/// 999 HP, then the map move into the first of the act's two bosses at `hp`; `None` if the walk
/// ends the run first.
fn boss3_snapshot(seed: u64, hp: i32) -> Option<Snapshot> {
    let config = RunConfig {
        character: Some(sts2_core::Character::Necrobinder),
        ascension: 10,
        ..config(CombatMode::Greedy)
    };
    let mut run = config.run(seed).expect("a run");
    let mut greedy = CombatMode::Greedy.resolver().expect("greedy");
    greedy.reset(seed);
    let mut rng = SplitMix::new(seed);
    let mut entered = false;
    for _ in 0..20_000 {
        if let RunDecision::Map(options) = run.pending_decision() {
            if !entered {
                run.debug_enter_act(2).expect("act 3");
                let s = run.loadout_state().expect("on the map");
                (s.max_hp, s.hp) = (999, 999);
                entered = true;
                continue;
            }
            let s = run.state();
            assert!(s.map.second_boss().is_some(), "A10 has two act-3 bosses");
            let boss = s.map.boss().coord;
            if options.contains(&boss) {
                let choice = RunAction::ChooseNode(boss);
                let mut snap = practice::capture(&run, &choice).expect("a boss snapshot");
                assert_eq!((snap.cell, snap.end), (BOSS3, SegmentEnd::Terminal));
                let s = snap.run.loadout_state().expect("on the map");
                (s.max_hp, s.hp) = (hp.max(80), hp);
                return Some(snap);
            }
            let pick = options[rng.below(options.len())];
            run.apply(RunAction::ChooseNode(pick)).expect("a map move");
            continue;
        }
        if run
            .combat()
            .is_some_and(|c| c.state().player.turn_number > 60)
        {
            return None;
        }
        let legal = run.legal_actions();
        if matches!(run.pending_decision(), RunDecision::Combat(_)) {
            let a = greedy.act(&run, &legal);
            if run.apply(a).is_ok() {
                continue;
            }
        }
        let a = legal.into_iter().find(|a| run.accepts(a))?;
        run.apply(a).expect("accepted");
    }
    None
}

/// How the `BOSS_ACT3` practice episodes of `snaps` ended (by [`RunEnd`] code) and the bosses
/// each killed (0, 1, 2), over at most `steps` random steps of 8 envs (all practice slots), until
/// `want` ended.
fn boss3_ends(snaps: Vec<Snapshot>, steps: usize, want: u64) -> ([u64; 9], [u64; 3]) {
    let n = 8;
    let mut envs = RunEnvs::new(
        n,
        SeedSource::Range {
            start: 0,
            end: 100_000,
            cycle: false,
        },
        RunConfig {
            ascension: 10,
            combat: CombatMode::Greedy,
            options: RunOptions {
                events: true,
                turn_cap: 100,
                ..RunOptions::OFF
            },
            ..RunConfig::default()
        },
    );
    let mut weights = [0; 6];
    weights[BOSS3.index()] = 1;
    envs.set_practice(Some(PracticeConfig {
        slots: n,
        weights,
        capture: false,
        ..practice(n)
    }));
    let pool = envs.practice_mut().expect("set");
    for snap in snaps {
        pool.add(snap);
    }
    // The first episodes are normal runs; practice starts at each env's first end.
    let mut rng = SplitMix::new(4);
    let (mut by_end, mut kills) = ([0; 9], [0; 3]);
    for _ in 0..steps {
        let actions: Vec<u16> = (0..n).map(|i| draw(envs.mask(i), &mut rng)).collect();
        envs.step_with(&actions, |ep| {
            if ep.cell.is_some() {
                assert_eq!(ep.cell, Some(BOSS3));
                by_end[ep.end as usize] += 1;
                kills[(ep.bosses as usize).min(2)] += 1;
            }
        })
        .expect("drawn from the masks");
        if by_end.iter().sum::<u64>() >= want {
            break;
        }
    }
    (by_end, kills)
}

/// RLN34: a `BOSS_ACT3` start (A10) plays both of the act's bosses to the run's end. It ends in a
/// death, a victory or the fight cap, never stuck, capped or unsupported; a victory killed both
/// bosses, so a win in this cell is two boss kills, not the first boss's.
#[test]
fn act3_boss_practice_plays_both_bosses_to_the_runs_end() {
    let base: Vec<Snapshot> = (700..)
        .filter_map(|s| boss3_snapshot(s, 1))
        .take(3)
        .collect();
    for hp in [99_999, 1] {
        let snaps: Vec<Snapshot> = base
            .iter()
            .map(|snap| {
                let mut snap = snap.clone();
                let s = snap.run.loadout_state().expect("on the map");
                (s.max_hp, s.hp) = (hp.max(80), hp);
                snap
            })
            .collect();
        let (ends, bosses) = boss3_ends(snaps, 2_000, 8);
        let total: u64 = ends.iter().sum();
        eprintln!("hp {hp}: ends (by RunEnd code) {ends:?}, bosses killed 0/1/2 {bosses:?}");
        assert!(total >= 8, "hp {hp}: {ends:?}");
        let ok = [RunEnd::Died, RunEnd::Victory, RunEnd::FightCapped];
        assert_eq!(
            ok.iter().map(|&e| ends[e as usize]).sum::<u64>(),
            total,
            "hp {hp}: {ends:?}"
        );
        if hp == 1 {
            assert_eq!(ends[RunEnd::Died as usize], total, "1 HP dies");
            assert_eq!(bosses[0], total, "1 HP kills no boss");
        } else {
            assert!(
                ends[RunEnd::Victory as usize] > 0,
                "99,999 HP wins: {ends:?}"
            );
            assert!(
                bosses[2] >= ends[RunEnd::Victory as usize],
                "a win kills both bosses"
            );
        }
    }
}
