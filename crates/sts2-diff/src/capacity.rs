//! The capacity survey's recorder (`docs/specs/rl-2x-plan.md`, stage 0): the high-water marks
//! ([`HighWater`]) of the combats a command plays.
//!
//! Off unless `sweep --capacities` turns it on or `STS2_CAPACITIES` names a directory; with the
//! variable, every process writes its marks to `<dir>/<pid>.txt` when it ends ([`finish`]), so
//! `tools/regress.sh` and `tools/replay_history.py` (many processes) can be surveyed too, and
//! `sts2-diff capacities <dir>` merges the files into one table. Each thread keeps its own marks
//! and merges them when it ends ([`flush_thread`]), so sampling takes no lock.

use std::cell::RefCell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use sts2_core::Combat;
use sts2_core::capacity::HighWater;
use sts2_core::run::RunState;

use crate::Result;

/// The variable naming the directory each process writes its marks to.
const DIR_VAR: &str = "STS2_CAPACITIES";

static ENABLED: AtomicBool = AtomicBool::new(false);
static TOTAL: Mutex<Option<HighWater>> = Mutex::new(None);

thread_local! {
    static LOCAL: RefCell<HighWater> = RefCell::new(HighWater::default());
}

/// Turns recording on; it is also on when `STS2_CAPACITIES` is set.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

/// Whether combats are recorded.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Turns recording on if `STS2_CAPACITIES` is set (the CLI calls it once at start-up).
pub fn enable_from_env() {
    if std::env::var_os(DIR_VAR).is_some() {
        enable();
    }
}

/// Samples `combat` if recording is on; `label` names it (seed, act) when a mark rises.
pub fn observe(combat: &Combat, label: impl Fn() -> String) {
    if enabled() {
        LOCAL.with(|l| l.borrow_mut().observe(combat, label));
    }
}

/// Samples a run's own lists if recording is on.
pub fn observe_run(state: &RunState, label: impl Fn() -> String) {
    if enabled() {
        LOCAL.with(|l| l.borrow_mut().observe_run(state, label));
    }
}

/// Merges this thread's marks into the process's.
///
/// # Panics
/// If another thread panicked while merging.
pub fn flush_thread() {
    if !enabled() {
        return;
    }
    let mine = LOCAL.with(|l| std::mem::take(&mut *l.borrow_mut()));
    TOTAL
        .lock()
        .expect("no panic while merging")
        .get_or_insert_with(HighWater::default)
        .merge(&mine);
}

/// The process's marks so far (this thread's merged first).
///
/// # Panics
/// If another thread panicked while merging.
pub fn take() -> HighWater {
    flush_thread();
    TOTAL
        .lock()
        .expect("no panic while merging")
        .take()
        .unwrap_or_default()
}

/// Writes the process's marks to `$STS2_CAPACITIES/<pid>.txt`, if the variable is set.
///
/// # Errors
/// If the file cannot be written.
pub fn finish() -> Result<()> {
    let Some(dir) = std::env::var_os(DIR_VAR) else {
        return Ok(());
    };
    let marks = take();
    if marks.samples == 0 {
        return Ok(());
    }
    std::fs::create_dir_all(&dir)?;
    let path = std::path::Path::new(&dir).join(format!("{}.txt", std::process::id()));
    std::fs::write(path, marks.to_lines())?;
    Ok(())
}

/// The marks of every file in `dir`, merged.
///
/// # Errors
/// If `dir` cannot be read.
pub fn merge_dir(dir: &str) -> Result<(HighWater, usize)> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|x| x == "txt") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut total = HighWater::default();
    for path in &paths {
        total.merge(&HighWater::parse(&std::fs::read_to_string(path)?));
    }
    Ok((total, paths.len()))
}
