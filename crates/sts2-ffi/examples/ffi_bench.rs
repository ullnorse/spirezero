//! Steps per second through the C ABI from Rust: the loop `python/bench.py` runs, without
//! Python. `cargo run --profile ffi -p sts2-ffi --example ffi_bench -- [ENVS] [STEPS] [THREADS]
//! [SEED]` (defaults 64, 20,000, 1, 0) on the default W1-all source; the policy is
//! `python/sts2/policy.py`'s (env `i` draws from `SplitMix64(seed + i)` and plays that rank of
//! its set mask bits), so both print the same actions, episodes, wins and observation digest.
#![allow(
    clippy::print_stdout,
    clippy::multiple_unsafe_ops_per_block,
    reason = "a bench binary that reports on stdout and reads the library's pointers"
)]

use std::ptr;
use std::time::Instant;

use sts2_ffi::{
    Sts2StepStats, sts2_envs_free, sts2_envs_len, sts2_envs_mask_ptr, sts2_envs_new,
    sts2_envs_obs_ptr, sts2_envs_stats, sts2_envs_step, sts2_envs_won_ptr,
};
use sts2_rl::action::nth_set;
use sts2_rl::{MASK_WORDS, OBS_BYTES};

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
    let steps = arg(1, 20_000);
    let threads = usize::try_from(arg(2, 1)).expect("a thread count");
    let seed = arg(3, 0);
    // SAFETY: a null path selects the default source.
    let envs = unsafe { sts2_envs_new(envs_n, ptr::null(), seed, threads) };
    assert!(!envs.is_null(), "sts2_envs_new failed");
    // SAFETY: `envs` is live; the buffers have `n × MASK_WORDS` words, `n × OBS_BYTES` and `n`
    // bytes and keep their address until `sts2_envs_free`.
    let (n, masks, obs, won) = unsafe {
        let n = sts2_envs_len(envs);
        (
            n,
            sts2_envs_mask_ptr(envs),
            sts2_envs_obs_ptr(envs),
            sts2_envs_won_ptr(envs),
        )
    };
    let mut policy: Vec<u64> = (0..).map(|i: u64| seed + i).take(n).collect();
    let mut actions = vec![0_u16; n];
    let (mut applied, mut episodes, mut wins) = (0_u64, 0_u64, 0_u64);
    let mut stats = Sts2StepStats::default();
    let start = Instant::now();
    for _ in 0..steps {
        // SAFETY: as above; nothing writes the masks during the read.
        let masks = unsafe { std::slice::from_raw_parts(masks, n * MASK_WORDS) };
        for ((action, state), mask) in actions
            .iter_mut()
            .zip(&mut policy)
            .zip(masks.as_chunks::<MASK_WORDS>().0)
        {
            let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
            let nth = next(state) % u64::from(count.max(1));
            let index = nth_set(mask, u32::try_from(nth).expect("below 140")).unwrap_or(0);
            *action = u16::try_from(index).expect("below 140");
        }
        // SAFETY: `actions` has `n` aligned `u16`s; `stats` is writable.
        let code = unsafe {
            let code = sts2_envs_step(envs, actions.as_ptr(), n);
            sts2_envs_stats(envs, &raw mut stats);
            code
        };
        assert!(code >= 0, "step failed: {code}");
        applied += stats.actions;
        episodes += u64::from(stats.ended);
        // SAFETY: the won buffer has `n` bytes.
        let won = unsafe { std::slice::from_raw_parts(won, n) };
        wins += won.iter().map(|&w| u64::from(w)).sum::<u64>();
    }
    let seconds = start.elapsed().as_secs_f64();
    // SAFETY: the obs buffer has `n × OBS_BYTES` bytes.
    let digest = fnv(unsafe { std::slice::from_raw_parts(obs, n * OBS_BYTES) });
    #[expect(
        clippy::cast_precision_loss,
        clippy::float_arithmetic,
        reason = "a rate for the report"
    )]
    let rate = applied as f64 / seconds;
    println!(
        "rust-ffi envs={n} threads={threads} steps={steps} actions={applied} episodes={episodes} \
         wins={wins} obs_digest={digest:016x} seconds={seconds:.3} steps_per_s={rate:.0}"
    );
    // SAFETY: `envs` is freed once.
    unsafe { sts2_envs_free(envs) };
}
