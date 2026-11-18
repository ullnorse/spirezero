//! RLN14: the members of a merged action class look the same to the agent. The run env counts,
//! per observation field ([`RECORD_FIELDS`]), merged members that differ from their class's
//! first member (debug builds panic on one); here random play over Necrobinder A10 finds none,
//! and `member_sweep` (ignored, release) is the big sweep:
//! `SWEEP_DECISIONS=300000 SWEEP_SEED=162000000 cargo test --release -p sts2-rl --test members -- --ignored --nocapture`.

use sts2_core::combat::DecisionRequest;
use sts2_rl::action;
use sts2_rl::obs::RECORD_FIELDS;
use sts2_rl::seeds::SplitMix;
use sts2_rl::{CombatMode, RunConfig, RunEnvs, RunOptions, SeedSource};

fn draw(mask: &[u64], rng: &mut SplitMix) -> u16 {
    let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
    if count == 0 {
        return 0;
    }
    let nth = u32::try_from(rng.below(count as usize)).expect("below count");
    u16::try_from(action::nth_set(mask, nth).expect("below the count")).expect("small")
}

#[derive(Debug, Default)]
struct Sweep {
    decisions: u64,
    merged: u64,
    mismatch: [u64; RECORD_FIELDS.len()],
    choices: u64,
    mixed_piles: u64,
}

fn sweep(seed: u64, decisions: u64, pile_key: bool) -> Sweep {
    let n = 16;
    let config = RunConfig {
        ascension: 10,
        combat: CombatMode::Agent,
        options: RunOptions {
            pile_key,
            ..RunOptions::E2E
        },
        ..RunConfig::default()
    };
    let source = SeedSource::Range {
        start: seed,
        end: seed + 1_000_000,
        cycle: false,
    };
    let mut envs = RunEnvs::new(n, source, config);
    let mut rng = SplitMix::new(seed);
    let mut out = Sweep::default();
    while out.decisions < decisions && envs.live() > 0 {
        for i in 0..n {
            let Some(combat) = envs.run(i).and_then(|r| r.combat()) else {
                continue;
            };
            if let Some(DecisionRequest::ChooseCards { options, .. }) = combat.pending_decision() {
                out.choices += 1;
                let first = combat.state().pile_of(options[0]);
                out.mixed_piles +=
                    u64::from(options.iter().any(|&c| combat.state().pile_of(c) != first));
            }
        }
        let actions: Vec<u16> = (0..n).map(|i| draw(envs.mask(i), &mut rng)).collect();
        let step = envs.step(&actions).expect("legal");
        out.decisions += step.decisions;
        out.merged += step.merged;
        for (a, b) in out.mismatch.iter_mut().zip(step.member_mismatch) {
            *a += b;
        }
    }
    out
}

#[test]
fn merged_members_look_alike() {
    let s = sweep(162_000_000, 20_000, false);
    assert!(s.merged > 0, "{s:?}");
    assert_eq!(s.mismatch, [0; RECORD_FIELDS.len()], "{s:?}");
}

#[test]
#[ignore = "the big sweep; run in release"]
fn member_sweep() {
    let decisions = std::env::var("SWEEP_DECISIONS").map_or(300_000, |v| v.parse().expect("n"));
    let seed = std::env::var("SWEEP_SEED").map_or(162_000_000, |v| v.parse().expect("seed"));
    for pile_key in [false, true] {
        let s = sweep(seed, decisions, pile_key);
        println!("pile_key={pile_key}: {s:?} fields {RECORD_FIELDS:?}");
    }
}

/// The counter itself, on toy actions `(index, class, field bits)`: a merged member that differs
/// from the class's first member in a field is counted under that field, one that matches is not.
#[test]
fn the_counter_counts_per_field() {
    use sts2_rl::action::Mask;
    use sts2_rl::classes::{ClassKey, DedupeScratch};
    let legal: [(usize, &'static str, u8); 5] = [
        (0, "a", 0b00100), // first of class a
        (1, "a", 0b00100), // same look: no mismatch
        (2, "a", 0b00001), // differs from the first in "kind"; the check reads the diff bits
        (3, "b", 0),
        (4, "b", 0),
    ];
    let mut mask = Mask::new();
    mask.fill_with(&legal, |a| Some(a.0));
    let mut counts = vec![0; action::ACTIONS];
    let d = mask.dedupe_checked(
        &legal,
        |a| Some(a.0),
        |a| Some(ClassKey::BeltPotion(a.1)),
        |a, b| a == b,
        |_| true,
        |first, member| first.2 ^ member.2,
        &mut counts,
        &mut DedupeScratch::default(),
    );
    assert_eq!(d.merged, 3);
    assert_eq!(d.mismatches, [1, 0, 1, 0, 0]);
}
