//! The honest forecast (RLN16, `sts2_rl::obs_v2`): a move forecast as certain is the move the
//! monster rolls next; and the v2
//! section through the run env changes neither the v1 bytes, the masks nor the episodes.

use std::collections::HashMap;

use sts2_core::Status;
use sts2_core::combat::CreatureId;
use sts2_rl::run_obs::{RUN_OBS_BYTES, V1_BYTES, V2_AT};
use sts2_rl::{RunConfig, RunEnvs, RunOptions, SeedSource, Start};

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

/// Each monster's step-1 forecast: the move's id, and whether it is certain.
fn forecasts(combat: &sts2_core::Combat) -> HashMap<CreatureId, (&'static str, bool)> {
    let s = combat.state();
    s.enemies
        .iter()
        .filter_map(|&id| {
            let m = s.creature(id).monster.as_ref()?;
            let table = m.kind.moves();
            let f = m.machine.forecast(table, s, id, m.rolled, 1);
            let &(next, certainty) = f.first()?;
            #[expect(
                clippy::float_cmp,
                reason = "a product of shares that are all exactly 1"
            )]
            let certain = certainty == 1.0;
            Some((id, (table.states[next].id(), certain)))
        })
        .collect()
}

#[test]
fn a_certain_forecast_is_the_next_move() {
    let (mut certain, mut hits, mut uncertain, mut uncertain_hits) = (0, 0, 0, 0);
    let mut legal = Vec::new();
    for seed in 0..220 {
        let mut combat = Start::w1_all(seed).combat;
        let mut policy = Policy(seed ^ 0xF0CA);
        let mut last = HashMap::new();
        let mut turn = combat.state().player.turn_number;
        for _ in 0..200 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let s = combat.state();
            if s.player.turn_number != turn {
                // A new turn: each surviving monster rolled once, unless a forced move holds it.
                for (&id, &(name, sure)) in &last {
                    let Some(m) = s.creature(id).monster.as_ref() else {
                        continue;
                    };
                    if !s.creature(id).is_alive() || m.machine.forced.is_some() {
                        continue;
                    }
                    let came = m.next_move_name() == name;
                    if sure {
                        certain += 1;
                        hits += usize::from(came);
                    } else {
                        uncertain += 1;
                        uncertain_hits += usize::from(came);
                    }
                }
                turn = s.player.turn_number;
            }
            last = forecasts(&combat);
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            let count = u64::try_from(legal.len()).expect("fits u64");
            let nth = usize::try_from(policy.next() % count).expect("below count");
            combat
                .apply(legal.swap_remove(nth))
                .expect("a listed action applies");
        }
    }
    println!(
        "certain {hits}/{certain} came true; uncertain (heaviest branch) {uncertain_hits}/{uncertain}"
    );
    assert!(
        certain > 1_000 && uncertain > 200,
        "{certain} certain, {uncertain} uncertain"
    );
    // A conditional branch reads the board at the forecast and the roll reads it after the
    // enemy turn, so a certain forecast could miss; on these fights none does.
    assert_eq!(
        hits, certain,
        "{hits} of {certain} certain forecasts came true"
    );
}

fn envs(obs_v2: bool) -> RunEnvs {
    RunEnvs::new(
        4,
        SeedSource::Range {
            start: 0,
            end: 1_000,
            cycle: false,
        },
        RunConfig {
            options: RunOptions {
                obs_v2,
                priv_obs: true,
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
fn the_v2_section_changes_nothing_else() {
    let (mut off, mut on) = (envs(false), envs(true));
    let mut written = 0;
    for step in 0..3_000 {
        for (a, b) in off
            .obs()
            .chunks(RUN_OBS_BYTES)
            .zip(on.obs().chunks(RUN_OBS_BYTES))
        {
            assert_eq!(a[..V1_BYTES], b[..V1_BYTES], "step {step}");
            assert!(a[V2_AT..].iter().all(|&x| x == 0), "off is zero");
            written += usize::from(b[V2_AT..].iter().any(|&x| x != 0));
        }
        assert_eq!(off.masks(), on.masks(), "step {step}");
        assert_eq!(off.priv_obs(), on.priv_obs(), "step {step}");
        let actions = first_actions(&off);
        let a = off.step(&actions).expect("legal");
        let b = on.step(&actions).expect("legal");
        assert_eq!(a, b, "step {step}");
    }
    assert!(written > 1_000, "{written} observations with a v2 section");
}

/// RLN17: the forecast intents come from the intent cache, whose debug builds recompute every
/// hit and assert that it is the same ([`sts2_rl::intents`]'s oracle). Uniform random play at
/// A10, so turns run several decisions and the cache hits.
#[test]
fn cached_forecasts_match_a_recompute() {
    let mut e = RunEnvs::new(
        8,
        SeedSource::Range {
            start: 23_000_000,
            end: 23_100_000,
            cycle: false,
        },
        RunConfig {
            ascension: 10,
            options: RunOptions {
                obs_v2: true,
                ..RunOptions::E2E
            },
            ..RunConfig::default()
        },
    );
    let mut policy = Policy(17);
    let mut written = 0;
    for _ in 0..4_000 {
        written += e
            .obs()
            .chunks(RUN_OBS_BYTES)
            .filter(|row| row[V2_AT..].iter().any(|&x| x != 0))
            .count();
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
    assert!(written > 10_000, "{written} observations with a v2 section");
}
