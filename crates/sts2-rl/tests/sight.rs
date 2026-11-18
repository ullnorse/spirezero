//! WEB82: `RunSight` sees what the env shows its agent: at every decision of whole runs (the
//! CB3 net's options, the full view's history included), the same observation bytes and mask.

use sts2_rl::sight::RunSight;
use sts2_rl::{RUN_OBS_BYTES, RunConfig, RunEnvs, SeedSource};

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
fn the_sight_sees_what_the_env_shows() {
    let mut e = RunEnvs::new(
        2,
        SeedSource::Range {
            start: 41_000_000,
            end: 41_100_000,
            cycle: false,
        },
        RunConfig {
            ascension: 10,
            options: RunSight::cb3(),
            ..RunConfig::default()
        },
    );
    e.reset_all();
    let mut sights = [
        RunSight::new(RunSight::cb3()),
        RunSight::new(RunSight::cb3()),
    ];
    let mut policy = Policy(82);
    let (mut fights, mut episodes) = (0, 0);
    for step in 0..2_500 {
        for (i, sight) in sights.iter_mut().enumerate() {
            let run = e.run(i).expect("a live run");
            let classes = sight.settle(run);
            assert_eq!(
                sight.mask().bits(),
                e.mask(i),
                "step {step} env {i}: the mask"
            );
            assert!(
                classes > 1,
                "step {step} env {i}: the env showed {classes} classes"
            );
            let obs = &e.obs()[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES];
            let seen = sight.see(run);
            if let Some(at) = (0..RUN_OBS_BYTES).find(|&k| seen[k] != obs[k]) {
                panic!("step {step} env {i}: byte {at} differs");
            }
            fights += usize::from(run.combat().is_some());
        }
        let actions: Vec<u16> = (0..e.len())
            .map(|i| {
                let mask = e.mask(i);
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                let nth = u32::try_from(policy.next() % u64::from(count)).expect("below count");
                let index = sts2_rl::action::nth_set(mask, nth).expect("a set bit");
                u16::try_from(index).expect("an index")
            })
            .collect();
        e.step(&actions).expect("legal");
        for (i, sight) in sights.iter_mut().enumerate() {
            if e.done()[i] != 0 {
                sight.reset();
                episodes += 1;
            }
        }
    }
    assert!(fights > 2_000, "{fights} fight decisions");
    assert!(episodes > 0, "no episode ended");
}
