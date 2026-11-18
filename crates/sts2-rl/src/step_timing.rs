//! RLN25: an opt-in wall-time profile of the env threads (`STS2_STEP_TIMING=1`).
//!
//! Each parallel pass over the envs ([`crate::run_envs`]'s `each_env`) records its wall time,
//! the busiest thread's time and the threads' summed time; every [`REPORT_EVERY`] passes a line
//! goes to stderr: mean wall, mean busiest, mean average thread, all in µs, and the efficiency
//! (average thread over wall, per mille). Off, a pass costs one relaxed load.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Passes per stderr line.
const REPORT_EVERY: u64 = 256;

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os("STS2_STEP_TIMING").is_some_and(|v| v == "1"));

/// Passes, then µs sums: wall, busiest thread, all threads; thread count sum.
static SUMS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

/// Whether the profile is on.
pub fn enabled() -> bool {
    *ENABLED
}

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// Records one pass that started at `start`, with each thread's busy time.
pub fn record(start: Instant, busy: &[Duration]) {
    let wall = micros(start.elapsed());
    let max = busy.iter().copied().max().map_or(0, micros);
    let sum: u64 = busy.iter().copied().map(micros).sum();
    let threads = u64::try_from(busy.len()).unwrap_or(u64::MAX);
    let passes = SUMS[0].fetch_add(1, Ordering::Relaxed) + 1;
    for (s, v) in SUMS[1..].iter().zip([wall, max, sum, threads]) {
        s.fetch_add(v, Ordering::Relaxed);
    }
    if passes.is_multiple_of(REPORT_EVERY) {
        let [_, wall, max, sum, threads] = SUMS.each_ref().map(|s| s.swap(0, Ordering::Relaxed));
        let n = REPORT_EVERY;
        let avg = sum / threads.max(1);
        #[expect(clippy::print_stderr, reason = "an opt-in profile for a human")]
        {
            eprintln!(
                "step_timing: {n} passes, {} thr, wall {} us, busiest {} us, mean thread {} us, \
                 efficiency {}‰",
                threads / n,
                wall / n,
                max / n,
                avg,
                (sum * 1000) / (wall * threads / n).max(1),
            );
        }
    }
}
