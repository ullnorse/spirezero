//! The privileged buffer (`docs/plans/necro-e2e.md`, WP2) through the run env: off it is zero and
//! changes nothing; on it is written and the observations, masks and episodes are the same.

use sts2_rl::priv_obs::{DRAW_AT, MOVES_AT, NEXT_AT, PRIV_BYTES};
use sts2_rl::{CombatMode, RunConfig, RunEnvs, RunOptions, SeedSource};

fn envs(priv_obs: bool, combat: CombatMode) -> RunEnvs {
    RunEnvs::new(
        4,
        SeedSource::Range {
            start: 0,
            end: 1_000,
            cycle: false,
        },
        RunConfig {
            combat,
            options: RunOptions {
                priv_obs,
                ..RunOptions::E2E
            },
            ..RunConfig::default()
        },
    )
}

/// The first set bit of every env's mask.
fn first_actions(e: &RunEnvs) -> Vec<u16> {
    (0..e.len())
        .map(|i| {
            let mask = e.mask(i);
            let word = mask.iter().position(|&w| w != 0).unwrap_or(0);
            u16::try_from(word * 64 + mask[word].trailing_zeros() as usize).expect("an index")
        })
        .collect()
}

#[test]
fn priv_does_not_change_the_envs_and_is_zero_when_off() {
    let (mut off, mut on) = (
        envs(false, CombatMode::Agent),
        envs(true, CombatMode::Agent),
    );
    let (mut draw_seen, mut moves_seen, mut next_seen) = (false, false, false);
    for step in 0..1_500 {
        assert_eq!(off.obs(), on.obs(), "step {step}");
        assert_eq!(off.masks(), on.masks(), "step {step}");
        assert!(off.priv_obs().iter().all(|&b| b == 0));
        assert_eq!(on.priv_obs().len(), 4 * PRIV_BYTES);
        for p in on.priv_obs().chunks(PRIV_BYTES) {
            draw_seen |= p[DRAW_AT..MOVES_AT].iter().any(|&b| b != 0);
            moves_seen |= p[MOVES_AT..MOVES_AT + 32].iter().any(|&b| b != 0);
            next_seen |= p[NEXT_AT..].iter().any(|&b| b != 0);
        }
        let actions = first_actions(&off);
        assert_eq!(actions, first_actions(&on));
        let a = off.step(&actions).expect("legal");
        let b = on.step(&actions).expect("legal");
        assert_eq!(a, b, "step {step}");
    }
    assert!(
        draw_seen && moves_seen && next_seen,
        "{draw_seen} {moves_seen} {next_seen}"
    );
}
