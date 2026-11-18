//! WEB83: the browser's combat-net path end to end against the training env's.
//!
//! The training env is [`RunEnvs`] with CB3 lane L's options (what `train_e2e.py` and
//! `naz3_combat.play` build through the FFI: [`RunSight::cb3`] plus the privileged buffers).
//! Two checks, decision by decision: observation bytes, mask words and the game action each
//! chosen index maps to must be the same.
//!
//! - `gym_fights_match_the_env`: the human panel's fights (`naz3_combat.play`'s gym), each
//!   stepped by the env and by a [`RunSight`] on a copy of the env's run at the fight's start.
//! - `web_runs_match_the_env`: whole runs, the env's on seed `s` and the page's ([`CombatNet`]
//!   on a [`Game`] replayed from its action list) on `RUN{s}`, at ascension 10.
//!
//! Both also report how often the net plays a Strike over a playable Unleash, and with
//! `WEB83_LETHAL=1` the turns it had a lethal line and did not take it ([`lethal`], slow; the
//! missed turns go to `target/web83-lethal-{gym,web}.bin` for `python/web83_value.py`). Knobs:
//! `WEB83_FIGHTS` (panel fights, 48), `WEB83_RUNS`/`WEB83_FIRST`/`WEB83_ASC` (whole runs: 6 from
//! seed 1 at A10), `WEB83_SHOW` (print every Strike-over-Unleash pick), `WEB83_SAVE` (print the
//! page's save at turn-1 ones, to load in the browser). Local data
//! (`~/.cache/spirebot-data/cb1/panel_bench.jsonl`, `net/cb3.bin`); skipped when missing. Run
//! with `--release`.

#![allow(
    clippy::float_arithmetic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "a parity test that reports what it found"
)]

use std::path::PathBuf;

mod lethal;

use lethal::Study;

use sts2_core::combat::Action;
use sts2_core::content::powers::PowerKind;
use sts2_core::run::{Run, RunAction, RunDecision};
use sts2_rl::RunEnd;
use sts2_rl::priv_obs::PRIV_BYTES;
use sts2_rl::sight::RunSight;
use sts2_rl::{
    CombatMode, Gym, GymConfig, HOLD, RUN_MASK_WORDS, RUN_OBS_BYTES, RunConfig, RunEnvs, SeedSource,
};
use sts2_web::Game;
use sts2_web_bot::fights::CombatNet;
use sts2_web_bot::net::Net;
use sts2_web_bot::net::encode::encode;

fn data(rel: &str) -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME");
    PathBuf::from(home).join(".cache/spirebot-data").join(rel)
}

/// CB3 lane L's env: `Sts2RunEnvs(..., priv=True, obs_v2, obs_full, discard_potion,
/// plan_depth=1, **E2E)`.
const fn config(ascension: u8) -> RunConfig {
    let mut options = RunSight::cb3();
    options.priv_obs = true;
    RunConfig {
        character: Some(sts2_core::content::characters::Character::Necrobinder),
        ascension,
        combat: CombatMode::Agent,
        intents: true,
        options,
    }
}

fn is_card(run: &Run, action: &RunAction, name: &str) -> bool {
    let (RunAction::Combat(Action::PlayCard { card, .. }), Some(c)) = (action, run.combat()) else {
        return false;
    };
    format!("{:?}", c.state().card(*card).kind).starts_with(name)
}

/// The Strike-over-Unleash tally.
#[derive(Default, Debug)]
struct Tally {
    /// Decisions with a playable Unleash and a playable Strike.
    both: usize,
    /// ... where the net plays the Strike.
    strike: usize,
    /// ... and of those, with the player's Strength at most 2.
    strike_low_str: usize,
    /// ... and Osty at 3 HP or more (Unleash then hits for 9 or more against Strike's 6).
    strike_low_str_osty3: usize,
    /// ... where it plays the Unleash.
    unleash: usize,
    /// ... where the policy puts more weight on a Strike than on every Unleash.
    prefers_strike: usize,
    /// The run's first fight, turn 1 (the starting deck): decisions, and Strike played.
    first_turn: usize,
    first_turn_strike: usize,
}

impl Tally {
    /// Counts a decision; prints it when `show`.
    fn see(
        &mut self,
        run: &Run,
        legal: &[(usize, RunAction)],
        logits: &[f32],
        pick: usize,
        show: bool,
    ) -> bool {
        let strikes: Vec<_> = legal
            .iter()
            .filter(|(_, a)| is_card(run, a, "Strike"))
            .collect();
        let unleashes: Vec<_> = legal
            .iter()
            .filter(|(_, a)| is_card(run, a, "Unleash"))
            .collect();
        if strikes.is_empty() || unleashes.is_empty() {
            return false;
        }
        self.both += 1;
        let picked = legal.iter().find(|(i, _)| *i == pick).map(|(_, a)| a);
        let state = run.combat().unwrap().state();
        let strength = state
            .creature(state.player.creature)
            .power_amount(PowerKind::Strength);
        let strike = picked.is_some_and(|a| is_card(run, a, "Strike"));
        let show = show || (strike && std::env::var_os("WEB83_SHOW").is_some());
        let best = |set: &[&(usize, RunAction)]| {
            set.iter()
                .map(|(i, _)| logits[*i])
                .fold(f32::NEG_INFINITY, f32::max)
        };
        self.prefers_strike += usize::from(best(&strikes) > best(&unleashes));
        if state.player.turn_number == 1 && run.state().total_floor() <= 2 {
            self.first_turn += 1;
            self.first_turn_strike += usize::from(strike);
        }
        if strike {
            self.strike += 1;
            self.strike_low_str += usize::from(strength <= 2);
            let osty = state
                .osty()
                .map(|o| state.creature(o))
                .filter(|c| c.is_alive());
            self.strike_low_str_osty3 +=
                usize::from(strength <= 2 && osty.is_some_and(|c| c.hp >= 3));
        }
        if picked.is_some_and(|a| is_card(run, a, "Unleash")) {
            self.unleash += 1;
        }
        if show {
            let osty = state
                .osty()
                .map(|o| state.creature(o))
                .map(|c| (c.hp, c.is_alive()));
            let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let sum: f32 = logits
                .iter()
                .filter(|l| l.is_finite())
                .map(|l| (l - max).exp())
                .sum();
            println!(
                "  turn {} energy {} strength {strength} osty {osty:?}",
                state.player.turn_number, state.player.energy
            );
            for (i, a) in legal {
                let name = match a {
                    RunAction::Combat(Action::PlayCard { card, target }) => {
                        let kind = state.card(*card).kind;
                        let classes: Vec<_> = kind.vars().iter().map(|v| v.class).collect();
                        let pv = state.card_preview(
                            *card,
                            None,
                            sts2_core::content::cards::PreviewMode::Normal,
                        );
                        format!("{kind:?} -> {target:?} vars {classes:?} preview {pv:?}")
                    }
                    other => format!("{other:?}"),
                };
                println!(
                    "    {}[{i:3}] p={:.3} {name}",
                    if *i == pick { '*' } else { ' ' },
                    (logits[*i] - max).exp() / sum
                );
            }
        }
        true
    }
}

fn legal_of(sight: &RunSight) -> Vec<(usize, RunAction)> {
    let mask = sight.mask();
    (0..mask.count())
        .filter_map(|k| mask.nth(k))
        .filter_map(|i| Some((i, sight.action(i)?.clone())))
        .collect()
}

fn load() -> Option<(String, Vec<u8>)> {
    let (bench, net) = (data("cb1/panel_bench.jsonl"), data("net/cb3.bin"));
    if let (Ok(b), Ok(n)) = (std::fs::read_to_string(&bench), std::fs::read(&net)) {
        Some((b, n))
    } else {
        println!(
            "e2e parity: no local data ({}, {}): skipped",
            bench.display(),
            net.display()
        );
        None
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow unoptimized: cargo test --release -p sts2-web-bot"
)]
fn gym_fights_match_the_env() {
    let Some((bench, weights)) = load() else {
        return;
    };
    let net = Net::from_bytes(&weights).unwrap();
    let starts = Gym::parse(&bench).unwrap();
    let fights = std::env::var("WEB83_FIGHTS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(48)
        .min(starts.len());
    let n = 8.min(fights);
    let mut envs = RunEnvs::new(
        n,
        SeedSource::Range {
            start: 0,
            end: 1,
            cycle: true,
        },
        config(10),
    );
    assert_eq!(envs.load_gym(starts), envs_loaded_hint(&bench));
    envs.set_gym(Some(GymConfig {
        first: 0,
        slots: n,
        weights: [0; 9],
        variants: false,
        hall_hp_pct: 0,
        sequential: true,
        seed: 7_000_000,
    }));
    envs.reset_all();
    let mut sights: Vec<Option<(Run, RunSight)>> = vec![None; n];
    let (mut started, mut decisions, mut diffs, mut tally) = (0, 0usize, 0usize, Tally::default());
    let mut shown = 0;
    let mut fresh = vec![true; n];
    let mut study = study_on().then(|| Study::new(n, 200));
    loop {
        let mut acts = vec![HOLD; n];
        for i in 0..n {
            let Some(run) = envs.run(i) else { continue };
            if fresh[i] {
                if started >= fights {
                    continue;
                }
                started += 1;
                fresh[i] = false;
                let mut sight = RunSight::new(RunSight::cb3());
                sight.reset();
                assert!(sight.step(run), "the env shows only positions with choices");
                sights[i] = Some((run.clone(), sight));
            }
            let Some((copy, sight)) = sights[i].as_mut() else {
                continue;
            };
            if let Some(study) = study.as_mut()
                && run.combat().is_some()
            {
                study.decision(i, run, &env_row(&envs, i));
            }
            let obs = &envs.obs()[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES];
            let mask = envs.mask(i);
            decisions += 1;
            let mut same = sight.obs() == obs && sight.mask().bits()[..] == mask[..RUN_MASK_WORDS];
            let tok = encode(obs, mask).unwrap();
            let logits = net.logits(&tok);
            let pick = Net::greedy(&logits).unwrap();
            let env_action = envs.action(i, pick).unwrap();
            same &= sight.action(pick) == Some(env_action);
            if !same {
                diffs += 1;
                if diffs <= 5 {
                    let first = sight.obs().iter().zip(obs).position(|(a, b)| a != b);
                    println!(
                        "diff env {i}: first obs byte {first:?}, sight {:?} env {env_action:?}",
                        sight.action(pick)
                    );
                }
            }
            let show = shown < 3
                && copy
                    .combat()
                    .is_some_and(|c| c.state().player.turn_number == 1);
            if tally.see(copy, &legal_of(sight), &logits, pick, show) && show {
                shown += 1;
            }
            // The copy follows: the pick, then the single classes the env takes itself.
            copy.apply(sight.action(pick).unwrap().clone()).unwrap();
            while !matches!(
                copy.pending_decision(),
                RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
            ) && sight.settle(copy) == 1
            {
                let a = sight.action(sight.mask().nth(0).unwrap()).unwrap().clone();
                copy.apply(a).unwrap();
            }
            if sight.mask().count() > 1 {
                sight.see(copy);
            }
            acts[i] = pick as u16;
        }
        if acts.iter().all(|&a| a == HOLD) {
            break;
        }
        envs.step(&acts).unwrap();
        for i in 0..n {
            if envs.done()[i] != 0 {
                if let Some(study) = study.as_mut() {
                    study.end(i, !won_end(envs.ends()[i]));
                }
                fresh[i] = true;
                sights[i] = None;
            }
        }
    }
    println!(
        "gym e2e parity: {started} fights, {decisions} decisions, {diffs} differ; strike/unleash {tally:?}"
    );
    if let Some(study) = &study {
        println!("gym lethal: {}", study.report());
        write_rows(study, "web83-lethal-gym.bin");
    }
    assert_eq!(diffs, 0);
}

/// Whether the lethal study runs (`WEB83_LETHAL`; slow).
fn study_on() -> bool {
    std::env::var_os("WEB83_LETHAL").is_some()
}

/// Env `i`'s row for the value check: obs, mask words, privileged bytes.
fn env_row(envs: &RunEnvs, i: usize) -> Vec<u8> {
    let mut row = envs.obs()[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES].to_vec();
    for w in envs.mask(i) {
        row.extend_from_slice(&w.to_le_bytes());
    }
    row.extend_from_slice(&envs.priv_obs()[i * PRIV_BYTES..][..PRIV_BYTES]);
    row
}

/// Whether an episode end is a won fight.
fn won_end(end: u8) -> bool {
    [RunEnd::Victory, RunEnd::GymEnd, RunEnd::PracticeEnd]
        .iter()
        .any(|&e| e as u8 == end)
}

/// Writes the study's missed rows (`python/web83_value.py` reads them).
fn write_rows(study: &Study, name: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(name);
    let mut out = b"W83L".to_vec();
    for x in [study.rows.len(), RUN_OBS_BYTES, PRIV_BYTES] {
        out.extend_from_slice(&(x as u32).to_le_bytes());
    }
    for (cur, next, margin) in &study.rows {
        out.extend_from_slice(cur);
        out.extend_from_slice(next);
        out.extend_from_slice(&margin.to_le_bytes());
    }
    std::fs::write(&path, out).unwrap();
    println!("  {} missed rows -> {}", study.rows.len(), path.display());
}

/// Lines in the bench (what `load_gym` reports).
fn envs_loaded_hint(bench: &str) -> usize {
    bench.lines().filter(|l| !l.trim().is_empty()).count()
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow unoptimized: cargo test --release -p sts2-web-bot"
)]
fn web_runs_match_the_env() {
    let Some((_, weights)) = load() else {
        return;
    };
    let net = Net::from_bytes(&weights).unwrap();
    let asc: u8 = std::env::var("WEB83_ASC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let first: u64 = std::env::var("WEB83_FIRST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let runs: u64 = std::env::var("WEB83_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let (mut decisions, mut diffs, mut fights, mut tally, mut shown) =
        (0, 0, 0, Tally::default(), 0);
    let mut study = study_on().then(|| Study::new(1, 200));
    for seed in first..first + runs {
        let mut envs = RunEnvs::new(
            1,
            SeedSource::List {
                seeds: vec![seed],
                cycle: false,
            },
            config(asc),
        );
        let mut web = CombatNet::from_weights(&weights).unwrap();
        let mut game = Game::new(4, &format!("RUN{seed}"), asc, &[]).unwrap();
        assert_eq!(
            game.run().state().character,
            sts2_core::content::characters::Character::Necrobinder
        );
        let mut mirror = RunSight::new(RunSight::cb3());
        let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut fight_turn1 = false;
        let mut steps = 0;
        for _ in 0..3000 {
            if envs.done()[0] != 0 || envs.run(0).is_none() {
                break;
            }
            let obs = &envs.obs()[..RUN_OBS_BYTES];
            let mask = envs.mask(0).to_vec();
            let in_fight = matches!(game.run().pending_decision(), RunDecision::Combat(_));
            if let Some(study) = study.as_mut() {
                if in_fight {
                    study.decision(0, envs.run(0).unwrap(), &env_row(&envs, 0));
                } else {
                    study.end(0, false);
                }
            }
            let pick = if in_fight {
                decisions += 1;
                let tok = encode(obs, &mask).unwrap();
                let logits = net.logits(&tok);
                let pick = Net::greedy(&logits).unwrap();
                let env_action = envs.action(0, pick).unwrap().clone();
                let got = web
                    .pick(4, &format!("RUN{seed}"), asc, &[], game.actions())
                    .unwrap();
                let sight = web.sight();
                let same = sight.obs() == obs
                    && sight.mask().bits()[..] == mask[..]
                    && got.as_ref().map(|p| (&p.action, p.index)) == Some((&env_action, pick));
                if !same {
                    diffs += 1;
                    if diffs <= 5 {
                        let first = sight.obs().iter().zip(obs).position(|(a, b)| a != b);
                        println!(
                            "diff run {seed}: first obs byte {first:?}, masks equal {}, web {:?} env {env_action:?} ({pick})",
                            sight.mask().bits()[..] == mask[..],
                            got.as_ref().map(|p| (&p.action, p.index))
                        );
                    }
                }
                let turn1 = game
                    .run()
                    .combat()
                    .is_some_and(|c| c.state().player.turn_number == 1);
                if turn1 && !fight_turn1 {
                    fights += 1;
                }
                let print_save = turn1
                    && std::env::var_os("WEB83_SAVE").is_some()
                    && is_card(game.run(), &env_action, "Strike")
                    && legal_of(sight)
                        .iter()
                        .any(|(_, a)| is_card(game.run(), a, "Unleash"));
                if print_save {
                    println!(
                        "SAVE {}",
                        serde_json::json!({
                            "character": 4, "seed": format!("RUN{seed}"), "ascension": asc,
                            "modifiers": [], "actions": game.actions(),
                        })
                    );
                }
                let show = print_save || (turn1 && !fight_turn1 && shown < 3);
                fight_turn1 = turn1;
                if tally.see(game.run(), &legal_of(sight), &logits, pick, show) && show {
                    shown += 1;
                    println!(
                        "  (run {seed}, web picks {:?} p={:?})",
                        got.as_ref().map(|p| p.index),
                        got.as_ref().map(|p| p.prob)
                    );
                }
                pick
            } else {
                fight_turn1 = false;
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                let k =
                    (rng % u64::from(mask.iter().map(|w| w.count_ones()).sum::<u32>())) as usize;
                (0..mask.len() * 64)
                    .filter(|&i| mask[i / 64] >> (i % 64) & 1 == 1)
                    .nth(k)
                    .unwrap()
            };
            // The page's game plays the same action, then the single classes the env takes.
            let action = envs.action(0, pick).unwrap().clone();
            let index = game
                .recordable_actions()
                .iter()
                .position(|a| *a == action)
                .expect("recordable");
            game.advance(&[index as u32]).unwrap();
            while !game.is_over() && mirror.settle(game.run()) == 1 {
                let a = mirror
                    .action(mirror.mask().nth(0).unwrap())
                    .unwrap()
                    .clone();
                let index = game
                    .recordable_actions()
                    .iter()
                    .position(|b| *b == a)
                    .unwrap();
                game.advance(&[index as u32]).unwrap();
            }
            envs.step(&[pick as u16]).unwrap();
            steps += 1;
        }
        if let Some(study) = study.as_mut() {
            study.end(0, !won_end(envs.ends()[0]));
        }
        println!(
            "run {seed}: {steps} steps, end {:?}, web at {:?}",
            envs.ends()[0],
            game.run().pending_decision()
        );
    }
    println!(
        "web e2e parity: {runs} runs, {fights} fights, {decisions} fight decisions, {diffs} differ; strike/unleash {tally:?}"
    );
    if let Some(study) = &study {
        println!("web lethal: {}", study.report());
        write_rows(study, "web83-lethal-web.bin");
    }
    assert_eq!(diffs, 0);
}
