//! RLN22: the run env's plan section (`sts2_rl::obs_v2::PLAN_AT`): written on combat decisions
//! with `obs_v2`, only for the indices the mask holds, with at most one planner's choice, and
//! one whenever the turn can end. Its honesty is `tests/leak.rs`'s.

use sts2_rl::action::{ACTIONS, END_TURN};
use sts2_rl::obs_v2::{PLAN_AT, PLAN_BYTES, V2_AT};
use sts2_rl::plan_column::{PLAN_CHOSEN, PLAN_SCORED, PLAN_WINS};
use sts2_rl::run_obs::COMBAT_AT;
use sts2_rl::{RUN_OBS_BYTES, RunConfig, RunEnvs, RunOptions, SeedSource};

/// `SplitMix64`, the policy's RNG.
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

#[test]
fn the_plan_section_follows_the_mask() {
    let mut e = RunEnvs::new(
        8,
        SeedSource::Range {
            start: 22_000_000,
            end: 22_100_000,
            cycle: false,
        },
        RunConfig {
            ascension: 10,
            options: RunOptions {
                obs_v2: true,
                plan_column: true,
                ..RunOptions::E2E
            },
            ..RunConfig::default()
        },
    );
    let mut policy = Policy(22);
    let (mut fights, mut chosen_rows, mut wins) = (0, 0, 0);
    for step in 0..3_000 {
        for (i, row) in e.obs().chunks(RUN_OBS_BYTES).enumerate() {
            let plan = &row[V2_AT + PLAN_AT..][..ACTIONS * PLAN_BYTES];
            let mask = e.mask(i);
            let has = |index: usize| mask[index / 64] >> (index % 64) & 1 == 1;
            let in_fight = row[COMBAT_AT..].iter().any(|&x| x != 0);
            let mut chosen = 0;
            for (index, entry) in plan.chunks(PLAN_BYTES).enumerate() {
                let flags = entry[2];
                if flags == 0 {
                    assert_eq!(entry[..2], [0, 0], "step {step}: a score without flags");
                    continue;
                }
                assert!(in_fight, "step {step}: a plan entry outside a fight");
                assert!(flags & PLAN_SCORED != 0, "step {step}: flags {flags}");
                assert!(has(index), "step {step}: index {index} is not in the mask");
                chosen += usize::from(flags & PLAN_CHOSEN != 0);
                wins += usize::from(flags & PLAN_WINS != 0);
            }
            assert!(chosen <= 1, "step {step}: {chosen} choices");
            if in_fight && has(END_TURN) {
                assert_eq!(
                    chosen, 1,
                    "step {step}: the turn can end but nothing is chosen"
                );
                fights += 1;
            }
            chosen_rows += chosen;
        }
        let actions: Vec<u16> = (0..e.len())
            .map(|i| {
                let mask = e.mask(i);
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                if count == 0 {
                    return 0;
                }
                let nth = u32::try_from(policy.next() % u64::from(count)).expect("below count");
                let index = sts2_rl::action::nth_set(mask, nth).expect("a set bit");
                u16::try_from(index).expect("an index")
            })
            .collect();
        e.step(&actions).expect("legal");
    }
    assert!(fights > 2_000, "{fights} player-turn rows");
    assert!(chosen_rows >= fights, "{chosen_rows} rows with a choice");
    assert!(wins > 0, "no winning play was ever marked");
}

/// The plan column changes no other byte, mask or step: the same envs with it off and on.
#[test]
fn the_plan_column_changes_nothing_else() {
    let envs = |plan_column| {
        RunEnvs::new(
            4,
            SeedSource::Range {
                start: 0,
                end: 1_000,
                cycle: false,
            },
            RunConfig {
                options: RunOptions {
                    obs_v2: true,
                    plan_column,
                    priv_obs: true,
                    ..RunOptions::E2E
                },
                ..RunConfig::default()
            },
        )
    };
    let (mut off, mut on) = (envs(false), envs(true));
    let plan = V2_AT + PLAN_AT..V2_AT + PLAN_AT + ACTIONS * PLAN_BYTES;
    let mut policy = Policy(7);
    let mut written = 0;
    for step in 0..2_000 {
        for (a, b) in off
            .obs()
            .chunks(RUN_OBS_BYTES)
            .zip(on.obs().chunks(RUN_OBS_BYTES))
        {
            assert_eq!(a[..plan.start], b[..plan.start], "step {step}");
            assert_eq!(a[plan.end..], b[plan.end..], "step {step}");
            assert!(a[plan.clone()].iter().all(|&x| x == 0), "off is zero");
            written += usize::from(b[plan.clone()].iter().any(|&x| x != 0));
        }
        assert_eq!(off.masks(), on.masks(), "step {step}");
        assert_eq!(off.priv_obs(), on.priv_obs(), "step {step}");
        let actions: Vec<u16> = (0..off.len())
            .map(|i| {
                let mask = off.mask(i);
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                if count == 0 {
                    return 0;
                }
                let nth = u32::try_from(policy.next() % u64::from(count)).expect("below count");
                let index = sts2_rl::action::nth_set(mask, nth).expect("a set bit");
                u16::try_from(index).expect("an index")
            })
            .collect();
        let a = off.step(&actions).expect("legal");
        let b = on.step(&actions).expect("legal");
        assert_eq!(a, b, "step {step}");
    }
    assert!(written > 1_000, "{written} observations with a plan");
}
