//! Run-env steps and runs per second through the C ABI from Rust: the loop `python/run_bench.py`
//! runs, without Python. `cargo run --profile ffi -p sts2-ffi --example ffi_run_bench -- [ENVS]
//! [STEPS] [THREADS] [SEED] [COMBAT]` (defaults 64, 5,000, 1, 0, 0): Necrobinder runs at
//! ascension 0 on seeds drawn from master seed `SEED`, the fights played by the agent (`COMBAT`
//! 0) or settled by the greedy (1) or random (2) resolver. The policy is
//! `python/sts2/policy.py`'s (env `i` draws from `SplitMix64(seed + i)` and plays that rank of
//! its set mask bits), so both print the same decisions, actions, runs, wins and observation
//! digest. A step is one agent decision per env.
#![allow(
    clippy::print_stdout,
    clippy::multiple_unsafe_ops_per_block,
    reason = "a bench binary that reports on stdout and reads the library's pointers"
)]

use std::ptr;
use std::time::Instant;

use sts2_ffi::{
    Sts2RunConfig, Sts2RunStepStats, sts2_run_envs_free, sts2_run_envs_len, sts2_run_envs_mask_ptr,
    sts2_run_envs_new, sts2_run_envs_obs_ptr, sts2_run_envs_stats, sts2_run_envs_step,
    sts2_run_envs_won_ptr,
};
use sts2_rl::action::nth_set;
use sts2_rl::{RUN_MASK_WORDS, RUN_OBS_BYTES};

/// `SplitMix64`'s next value.
const fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a over `bytes`.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
    })
}

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("integer arguments"))
        .collect();
    let arg = |i: usize, default: u64| args.get(i).copied().unwrap_or(default);
    let envs_n = usize::try_from(arg(0, 64)).expect("an env count");
    let steps = arg(1, 5_000);
    let threads = usize::try_from(arg(2, 1)).expect("a thread count");
    let seed = arg(3, 0);
    let combat = u32::try_from(arg(4, 0)).expect("a combat mode");
    let config = Sts2RunConfig {
        seed_kind: 2,
        cycle: 0,
        seeds: ptr::null(),
        seeds_len: 0,
        start: 0,
        end: 0,
        master: seed,
        // Necrobinder, in select order.
        character: 4,
        ascension: 0,
        combat,
        intents: 1,
    };
    // SAFETY: `config` is readable.
    let envs = unsafe { sts2_run_envs_new(envs_n, &raw const config, threads) };
    assert!(!envs.is_null(), "sts2_run_envs_new failed");
    // SAFETY: `envs` is live; the buffers keep their address until `sts2_run_envs_free`.
    let (n, masks, obs, won) = unsafe {
        let n = sts2_run_envs_len(envs);
        (
            n,
            sts2_run_envs_mask_ptr(envs),
            sts2_run_envs_obs_ptr(envs),
            sts2_run_envs_won_ptr(envs),
        )
    };
    let mut policy: Vec<u64> = (0..).map(|i: u64| seed + i).take(n).collect();
    let mut actions = vec![0_u16; n];
    let (mut decisions, mut applied, mut runs, mut wins) = (0_u64, 0_u64, 0_u64, 0_u64);
    let mut stats = Sts2RunStepStats::default();
    let start = Instant::now();
    for _ in 0..steps {
        // SAFETY: the mask buffer has `n × RUN_MASK_WORDS` words; nothing writes it meanwhile.
        let masks = unsafe { std::slice::from_raw_parts(masks, n * RUN_MASK_WORDS) };
        for ((action, state), mask) in actions
            .iter_mut()
            .zip(&mut policy)
            .zip(masks.as_chunks::<RUN_MASK_WORDS>().0)
        {
            let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
            let nth = next(state) % u64::from(count.max(1));
            let index = nth_set(mask, u32::try_from(nth).expect("small")).unwrap_or(0);
            *action = u16::try_from(index).expect("small");
        }
        // SAFETY: `actions` has `n` aligned `u16`s; `stats` is writable.
        let code = unsafe {
            let code = sts2_run_envs_step(envs, actions.as_ptr(), n);
            sts2_run_envs_stats(envs, &raw mut stats);
            code
        };
        assert!(code >= 0, "step failed: {code}");
        decisions += stats.decisions;
        applied += stats.actions;
        runs += u64::from(stats.ended);
        // SAFETY: the won buffer has `n` bytes.
        let won = unsafe { std::slice::from_raw_parts(won, n) };
        wins += won.iter().map(|&w| u64::from(w)).sum::<u64>();
    }
    let seconds = start.elapsed().as_secs_f64();
    // SAFETY: the obs buffer has `n × RUN_OBS_BYTES` bytes.
    let digest = fnv(unsafe { std::slice::from_raw_parts(obs, n * RUN_OBS_BYTES) });
    #[expect(
        clippy::cast_precision_loss,
        clippy::float_arithmetic,
        reason = "rates for the report"
    )]
    let (steps_rate, runs_rate) = (decisions as f64 / seconds, runs as f64 / seconds);
    println!(
        "rust-ffi-runs envs={n} threads={threads} combat={combat} steps={steps} \
         decisions={decisions} actions={applied} runs={runs} wins={wins} obs_digest={digest:016x} \
         seconds={seconds:.3} steps_per_s={steps_rate:.0} runs_per_s={runs_rate:.1}"
    );
    // SAFETY: `envs` is freed once.
    unsafe { sts2_run_envs_free(envs) };
}
