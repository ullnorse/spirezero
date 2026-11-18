//! The C ABI over [`sts2_rl::Envs`] (`docs/specs/rl-2x-plan.md`, stage 8 and section 7.4) and
//! the run env [`sts2_rl::RunEnvs`] (stage 9, `sts2_run_envs_*`), for the Python `PufferEnv`
//! bindings in `python/sts2/`. The header is `include/sts2.h`.
//!
//! - **Buffers are engine-owned and stable.** A batch's observation, static observation, mask,
//!   `done` and `won` buffers are allocated once by [`sts2_envs_new`] and keep their address and
//!   size until [`sts2_envs_free`], so a caller wraps each once (numpy views, zero copy). They
//!   are written only during a call on the batch; read them between calls.
//! - **Errors are return codes**, never panics: every entry point runs under `catch_unwind`. A
//!   call returning a pointer returns null on error; one returning `i32` returns a negative
//!   `STS2_ERR_*`. [`sts2_last_error`] has the calling thread's last message. A panic inside a
//!   batch poisons it: every later call on it but [`sts2_envs_free`] returns
//!   [`STS2_ERR_POISONED`].
//! - **Build checks.** [`sts2_build_id`] is the hash of the sources the library was built from
//!   (`build.rs`), [`sts2_vocab_hash`] that of the id vocabularies the observation uses, and
//!   [`sts2_abi_version`] this interface's version; the Python wrapper refuses a library whose
//!   build id is not its checkout's.
//!
//! A batch is not thread-safe: one call at a time per batch. Distinct batches are independent.

#![deny(unsafe_op_in_unsafe_fn)]

use std::any::Any;
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;

use sts2_core::Character;
use sts2_rl::obs::{
    CARD_BYTES, CARDS_AT, CHOICE_AT, CREATURE_BYTES, CREATURES_AT, HAND_AT, INTENT_BYTES,
    INTENTS_AT,
};
use sts2_rl::priv_obs::PRIV_BYTES;
use sts2_rl::run_action::RUN_BASE;
use sts2_rl::run_obs::{
    ANCIENT_AT, BELT_AT, BUNDLES_AT, COMBAT_AT, COMBAT_STATIC_AT, DECK_AT, EVENT_AT, EVENTS_AT,
    FULL_AT, MAP_AT, RELICS_AT, REST_AT, REWARDS_AT, SELECT_AT, SHOP_AT, TREASURE_AT, V2_AT,
};
use sts2_rl::{
    ACTIONS, CombatMode, Envs, GYM_CELLS, Gym, GymConfig, HumanStart, MASK_WORDS, OBS_BYTES, Order,
    PracticeConfig, RUN_ACTIONS, RUN_MASK_WORDS, RUN_OBS_BYTES, RunConfig, RunEnvs, RunEvents,
    RunOptions, RunStepError, RunStepStats, STATIC_BYTES, SeedSource, Start, StartSource,
    StepError, StepStats, content_stats, vocab,
};

/// The version of this interface; it changes with any signature, layout or meaning.
///
/// 7 (RLN12) added practice starts (`sts2_run_envs_set_practice` and its buffers and stats); an
/// ABI-6 caller's calls behave as before.
///
/// 8 (RLN16) appended the run observation's v2 section (`Sts2RunLayout::v2_at`, written with
/// `Sts2RunConfig::obs_v2`): the run observation grew by `sts2_rl::obs_v2::V2_BYTES`, its v1
/// sections did not move, and both structs only grew at their ends.
///
/// 9 (RLN21) appended the facing and the powers' shown numbers to the v2 section
/// (`sts2_rl::obs_v2::SIDE_AT`, `DISPLAY_AT`): the run observation grew again at its end; nothing
/// before moved and no struct changed.
///
/// 10 (RLN22) appended the plan column to the v2 section (`sts2_rl::obs_v2::PLAN_AT`: per combat
/// action index a heuristic score and flags, written in fights with `obs_v2`): the run
/// observation grew at its end; nothing before moved and no struct changed.
///
/// 11 (RLN29) added the content stats (`sts2_content_stats`, `sts2_content_stats_columns`:
/// `sts2_rl::content_stats`); nothing else changed, so an ABI-10 caller's calls behave as before.
///
/// 12 (RLN30) appended the run observation's full-view section (`Sts2RunLayout::full_at`,
/// written with `Sts2RunConfig::obs_full` and `obs_v2`: `sts2_rl::obs_full`): the run
/// observation grew at its end, both structs only grew at their ends, and nothing before moved,
/// so an ABI-11 caller's calls behave as before.
///
/// 13 (RLN32) added the run actions that discard the potion in belt slot `s` (indices
/// `sts2_rl::run_action::DISCARD_POTION_BASE` + `s`, the space grew from 355 to 361 and
/// `Sts2RunLayout::actions` with it), listed only with `Sts2RunConfig::discard_potion`, and
/// `Sts2RunConfig::plan_depth` (plays per plan of the plan column). Every earlier index keeps its
/// number; both new fields 0 behave as ABI 12.
///
/// 14 (RLN33) appended the run observation's event section (`Sts2RunLayout::events_at`,
/// written with `Sts2RunConfig::obs_events`: `sts2_rl::obs_events`): the run observation grew at
/// its end, both structs only grew at their ends, and nothing before moved, so an ABI-13 caller's
/// calls behave as before.
///
/// 15 (RLN43) added the combat gym (`sts2_run_envs_load_gym`, `sts2_run_envs_set_gym`,
/// `sts2_run_envs_gym_stats`, `sts2_human_check`: `sts2_rl::human`), the run end 9 (`GymEnd`)
/// and cell codes 7-15 for gym episodes; nothing changes until a gym is loaded, so an ABI-14
/// caller's calls behave as before.
pub const STS2_ABI_VERSION: u32 = 15;

/// Success.
pub const STS2_OK: i32 = 0;
/// A required pointer was null.
pub const STS2_ERR_NULL: i32 = -1;
/// The action batch does not have one action per env.
pub const STS2_ERR_LENGTH: i32 = -2;
/// An action is not set in its env's mask; nothing was applied.
pub const STS2_ERR_ILLEGAL: i32 = -3;
/// An argument is invalid (no envs, an unreadable or empty start corpus, a bad path).
pub const STS2_ERR_ARGUMENT: i32 = -4;
/// The engine panicked; the batch is poisoned.
pub const STS2_ERR_PANIC: i32 = -5;
/// The batch panicked in an earlier call.
pub const STS2_ERR_POISONED: i32 = -6;

/// The W1-all starts of the default source (the RL bench's seeds).
pub const DEFAULT_STARTS: u64 = 2_200;

/// The source hash `build.rs` computed, NUL-terminated.
const BUILD_ID: &str = concat!(env!("STS2_SOURCE_HASH"), "\0");

/// What the last step of a batch did ([`StepStats`]).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2StepStats {
    /// Actions applied.
    pub actions: u64,
    /// Legal actions the fixed space has no index for, over the masks written.
    pub unmapped: u64,
    /// Episodes that ended; of those, won and capped at `MAX_EPISODE_ACTIONS`.
    pub ended: u32,
    pub won: u32,
    pub capped: u32,
    /// Envs reset to a new episode.
    pub reset: u32,
}

impl From<StepStats> for Sts2StepStats {
    fn from(s: StepStats) -> Self {
        Self {
            actions: s.actions,
            unmapped: s.unmapped,
            ended: s.ended,
            won: s.won,
            capped: s.capped,
            reset: s.reset,
        }
    }
}

/// The buffers' layout: sizes per env and the observation's section offsets
/// (`sts2_rl::obs`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2Layout {
    pub obs_bytes: u64,
    pub static_bytes: u64,
    pub mask_words: u64,
    pub actions: u64,
    pub creatures_at: u64,
    pub creature_bytes: u64,
    pub cards_at: u64,
    pub card_bytes: u64,
    pub hand_at: u64,
    pub intents_at: u64,
    pub intent_bytes: u64,
    pub choice_at: u64,
}

/// A batch of envs behind the C ABI.
#[derive(Debug)]
pub struct Sts2Envs {
    envs: Envs,
    stats: Sts2StepStats,
    poisoned: bool,
}

/// A batch behind the C ABI that a panic poisons.
trait Batch {
    fn poisoned(&mut self) -> &mut bool;
}

impl Batch for Sts2Envs {
    fn poisoned(&mut self) -> &mut bool {
        &mut self.poisoned
    }
}

impl Sts2Envs {
    /// The batch (for Rust callers of the rlib, such as the fuzz target).
    #[must_use]
    pub const fn envs(&self) -> &Envs {
        &self.envs
    }
}

thread_local! {
    /// The calling thread's last error message.
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// Records `message` as the calling thread's last error.
fn set_error(message: impl Into<String>) {
    let mut bytes = message.into().into_bytes();
    bytes.retain(|&b| b != 0);
    let message = CString::new(bytes).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = message);
}

/// The message of a panic's payload.
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(no message)")
}

/// Runs `f`; if it panics, records the panic and returns `on_panic` instead.
fn guard<T>(on_panic: T, f: impl FnOnce() -> T) -> T {
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(payload) => {
            set_error(format!("panic: {}", panic_message(&*payload)));
            on_panic
        }
    }
}

/// Runs `f` on the batch behind `envs` under `catch_unwind`; a panic poisons the batch.
///
/// # Safety
/// `envs` is null or a batch from [`sts2_envs_new`] not yet freed, and no other call uses it
/// meanwhile.
unsafe fn with_envs<B: Batch>(envs: *mut B, f: impl FnOnce(&mut B) -> i32) -> i32 {
    // SAFETY: by the caller's contract `envs` is null or a live, unaliased batch.
    let Some(handle) = (unsafe { envs.as_mut() }) else {
        set_error("the envs pointer is null");
        return STS2_ERR_NULL;
    };
    if *handle.poisoned() {
        set_error("the batch panicked in an earlier call");
        return STS2_ERR_POISONED;
    }
    match panic::catch_unwind(AssertUnwindSafe(|| f(handle))) {
        Ok(code) => code,
        Err(payload) => {
            *handle.poisoned() = true;
            set_error(format!("panic: {}", panic_message(&*payload)));
            STS2_ERR_PANIC
        }
    }
}

/// Reads a field of the batch behind `envs`; `none` if it is null.
///
/// # Safety
/// As [`with_envs`], except that a concurrent read-only call is allowed.
unsafe fn read_envs<B, T>(envs: *const B, none: T, f: impl FnOnce(&B) -> T) -> T {
    // SAFETY: by the caller's contract `envs` is null or a live batch nobody writes meanwhile.
    match unsafe { envs.as_ref() } {
        Some(handle) => guard(none, || f(handle)),
        None => none,
    }
}

/// The default source: the RL bench's W1-all starts, built once.
fn default_source() -> StartSource {
    static SOURCE: OnceLock<StartSource> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let starts = (0..DEFAULT_STARTS).map(Start::w1_all).collect();
            StartSource::new(starts, Order::Random)
        })
        .clone()
}

/// A boxed batch of `n` envs over `source` on `threads` threads, or null with the error set.
fn new_envs(
    n: usize,
    source: Result<StartSource, String>,
    seed: u64,
    threads: usize,
) -> *mut Sts2Envs {
    let source = match source {
        Ok(source) if !source.starts().is_empty() => source,
        Ok(_) => {
            set_error("the start source has no playable start");
            return std::ptr::null_mut();
        }
        Err(message) => {
            set_error(message);
            return std::ptr::null_mut();
        }
    };
    if n == 0 {
        set_error("a batch needs at least one env");
        return std::ptr::null_mut();
    }
    let mut envs = Envs::new(n, source, seed);
    envs.threads(threads);
    Box::into_raw(Box::new(Sts2Envs {
        envs,
        stats: Sts2StepStats::default(),
        poisoned: false,
    }))
}

/// [`STS2_ABI_VERSION`].
#[unsafe(no_mangle)]
pub const extern "C" fn sts2_abi_version() -> u32 {
    STS2_ABI_VERSION
}

/// The hash of the sources the library was built from, as 16 lowercase hex digits
/// (NUL-terminated, static).
#[unsafe(no_mangle)]
pub const extern "C" fn sts2_build_id() -> *const c_char {
    BUILD_ID.as_ptr().cast()
}

/// The hash of the id vocabularies the observation uses ([`vocab::hash`]).
#[unsafe(no_mangle)]
pub extern "C" fn sts2_vocab_hash() -> u64 {
    guard(0, vocab::hash)
}

/// The calling thread's last error message (NUL-terminated; empty if none). It stays valid
/// until the thread's next call into the library.
#[unsafe(no_mangle)]
pub extern "C" fn sts2_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

/// Writes the buffers' layout to `out`.
///
/// # Safety
/// `out` is null or valid for writing one [`Sts2Layout`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_layout(out: *mut Sts2Layout) -> i32 {
    let layout = Sts2Layout {
        obs_bytes: OBS_BYTES as u64,
        static_bytes: STATIC_BYTES as u64,
        mask_words: MASK_WORDS as u64,
        actions: ACTIONS as u64,
        creatures_at: CREATURES_AT as u64,
        creature_bytes: CREATURE_BYTES as u64,
        cards_at: CARDS_AT as u64,
        card_bytes: CARD_BYTES as u64,
        hand_at: HAND_AT as u64,
        intents_at: INTENTS_AT as u64,
        intent_bytes: INTENT_BYTES as u64,
        choice_at: CHOICE_AT as u64,
    };
    // SAFETY: by the caller's contract `out` is null or writable.
    let Some(out) = (unsafe { out.as_mut() }) else {
        set_error("the layout pointer is null");
        return STS2_ERR_NULL;
    };
    *out = layout;
    STS2_OK
}

/// A batch of `n` envs stepped on `threads` threads (0 is 1), reset to their first episodes.
///
/// Episodes start from the JSON-lines start corpus at `start_source_path` (`cargo xtask sweep
/// --emit-starts`), or from the RL bench's W1-all starts if it is null, in an order `seed`
/// draws. Null on error ([`sts2_last_error`]).
///
/// # Safety
/// `start_source_path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_new(
    n: usize,
    start_source_path: *const c_char,
    seed: u64,
    threads: usize,
) -> *mut Sts2Envs {
    let path = if start_source_path.is_null() {
        None
    } else {
        // SAFETY: by the caller's contract a non-null path is NUL-terminated.
        Some(unsafe { CStr::from_ptr(start_source_path) })
    };
    guard(std::ptr::null_mut(), || {
        let source = path.map_or_else(
            || Ok(default_source()),
            |path| {
                let path = path
                    .to_str()
                    .map_err(|_| "the start source path is not UTF-8".to_owned())?;
                let corpus = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                StartSource::parse(&corpus, Order::Random)
            },
        );
        new_envs(n, source, seed, threads)
    })
}

/// [`sts2_envs_new`] over a start corpus in memory: `corpus_len` bytes of JSON lines at
/// `corpus`.
///
/// # Safety
/// `corpus` is valid for reading `corpus_len` bytes (it may be null if `corpus_len` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_new_from_corpus(
    n: usize,
    corpus: *const u8,
    corpus_len: usize,
    seed: u64,
    threads: usize,
) -> *mut Sts2Envs {
    let bytes: &[u8] = if corpus.is_null() || corpus_len == 0 {
        &[]
    } else {
        // SAFETY: by the caller's contract `corpus` has `corpus_len` readable bytes.
        unsafe { std::slice::from_raw_parts(corpus, corpus_len) }
    };
    guard(std::ptr::null_mut(), || {
        let source = std::str::from_utf8(bytes)
            .map_err(|e| format!("the corpus is not UTF-8: {e}"))
            .and_then(|corpus| StartSource::parse(corpus, Order::Random));
        new_envs(n, source, seed, threads)
    })
}

/// Frees a batch (null is a no-op). Its buffers are gone afterwards.
///
/// # Safety
/// `envs` is null or a batch from [`sts2_envs_new`] not yet freed and not in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_free(envs: *mut Sts2Envs) {
    if envs.is_null() {
        return;
    }
    // SAFETY: by the caller's contract `envs` came from `Box::into_raw` in `new_envs` and is
    // freed once.
    let envs = unsafe { Box::from_raw(envs) };
    guard((), || drop(envs));
}

/// Applies `actions[i]` (an index of the action space) to env `i`, for every env still
/// playing, and resets the envs whose episode ended (`done`, `won`).
///
/// The number of episodes
/// that ended, or a negative `STS2_ERR_*`: [`STS2_ERR_LENGTH`] if `n` is not the env count,
/// [`STS2_ERR_ILLEGAL`] if a live env's action is not in its mask (nothing is applied then).
///
/// # Safety
/// `envs` as [`sts2_envs_free`]; `actions` is valid for reading `n` aligned `u16`s and is not
/// in one of the batch's buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_step(envs: *mut Sts2Envs, actions: *const u16, n: usize) -> i32 {
    if actions.is_null() {
        set_error("the actions pointer is null");
        return STS2_ERR_NULL;
    }
    // SAFETY: by the caller's contract `actions` has `n` readable, aligned `u16`s.
    let actions = unsafe { std::slice::from_raw_parts(actions, n) };
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| match handle.envs.step(actions) {
            Ok(stats) => {
                handle.stats = stats.into();
                i32::try_from(stats.ended).unwrap_or(i32::MAX)
            }
            Err(error) => {
                set_error(error.to_string());
                match error {
                    StepError::Length { .. } => STS2_ERR_LENGTH,
                    StepError::Illegal { .. } => STS2_ERR_ILLEGAL,
                }
            }
        })
    }
}

/// Starts a new episode in every env.
///
/// # Safety
/// As [`sts2_envs_step`]'s `envs`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_reset_all(envs: *mut Sts2Envs) -> i32 {
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            handle.envs.reset_all();
            handle.stats = Sts2StepStats::default();
            STS2_OK
        })
    }
}

/// Writes what the last step did to `out`.
///
/// # Safety
/// As [`sts2_envs_step`]'s `envs`; `out` is null or valid for writing one [`Sts2StepStats`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_stats(envs: *const Sts2Envs, out: *mut Sts2StepStats) -> i32 {
    // SAFETY: by the caller's contract `out` is null or writable.
    let Some(out) = (unsafe { out.as_mut() }) else {
        set_error("the stats pointer is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract `envs` is null or a live batch nobody writes meanwhile.
    unsafe {
        read_envs(envs, STS2_ERR_NULL, |handle| {
            *out = handle.stats;
            STS2_OK
        })
    }
}

/// Generates the read-only accessors of a batch.
macro_rules! accessor {
    ($(#[$doc:meta])* $name:ident -> $ty:ty, $none:expr, |$h:ident| $body:expr) => {
        accessor!($(#[$doc])* Sts2Envs, $name -> $ty, $none, |$h| $body);
    };
    ($(#[$doc:meta])* $batch:ty, $name:ident -> $ty:ty, $none:expr, |$h:ident| $body:expr) => {
        $(#[$doc])*
        ///
        /// # Safety
        /// `envs` is null or a batch from its `*_new` not yet freed.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(envs: *const $batch) -> $ty {
            // SAFETY: by the caller's contract `envs` is null or a live batch.
            unsafe { read_envs(envs, $none, |$h| $body) }
        }
    };
}

accessor!(
    /// The number of envs (0 for null).
    sts2_envs_len -> usize, 0, |h| h.envs.len()
);
accessor!(
    /// The observation bytes per env ([`OBS_BYTES`]; 0 for null).
    sts2_envs_obs_bytes -> usize, 0, |_h| OBS_BYTES
);
accessor!(
    /// The observations: `len × obs_bytes` bytes, env `i`'s at `i × obs_bytes` (null for null).
    sts2_envs_obs_ptr -> *const u8, std::ptr::null(), |h| h.envs.obs().as_ptr()
);
accessor!(
    /// The static observations, `len × static_bytes` bytes, written with the observations.
    sts2_envs_static_ptr -> *const u8, std::ptr::null(), |h| h.envs.static_obs().as_ptr()
);
accessor!(
    /// The masks: `len × mask_words` `u64`s; index `a` of env `i` is bit `a % 64` of word
    /// `i × mask_words + a / 64`.
    sts2_envs_mask_ptr -> *const u64, std::ptr::null(), |h| h.envs.masks().as_ptr()
);
accessor!(
    /// Per env, 1 when its episode ended in the last step (the env was reset since).
    sts2_envs_done_ptr -> *const u8, std::ptr::null(), |h| h.envs.done().as_ptr()
);
accessor!(
    /// Per env, 1 when its episode ended in the last step with a win.
    sts2_envs_won_ptr -> *const u8, std::ptr::null(), |h| h.envs.won().as_ptr()
);

accessor!(
    /// Per env, the player's HP (0 or more) when its episode ended in the last step.
    sts2_envs_final_hp_ptr -> *const i16, std::ptr::null(), |h| h.envs.final_hp().as_ptr()
);

/// From the next reset on, episodes start with what the player cannot see resampled from
/// `seed` and the episode's serial (`on` non-zero), or from their starts as they are.
///
/// # Safety
/// As [`sts2_envs_step`]'s `envs`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_envs_set_blindfold(envs: *mut Sts2Envs, on: u8, seed: u64) -> i32 {
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            handle.envs.blindfold((on != 0).then_some(seed));
            STS2_OK
        })
    }
}

/// A batch of run envs behind the C ABI.
#[derive(Debug)]
pub struct Sts2RunEnvs {
    envs: RunEnvs,
    stats: Sts2RunStepStats,
    poisoned: bool,
}

impl Sts2RunEnvs {
    /// The batch (for Rust callers of the rlib, such as the fuzz target).
    #[must_use]
    pub const fn envs(&self) -> &RunEnvs {
        &self.envs
    }
}

impl Batch for Sts2RunEnvs {
    fn poisoned(&mut self) -> &mut bool {
        &mut self.poisoned
    }
}

/// What a run batch plays ([`RunConfig`] and its [`SeedSource`]).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Sts2RunConfig {
    /// 0: the `seeds_len` seeds at `seeds`; 1: `start..end`; 2: random from `master`.
    pub seed_kind: u32,
    /// For a list or a range: 1 to start over at its end, 0 to stop.
    pub cycle: u32,
    pub seeds: *const u64,
    pub seeds_len: usize,
    pub start: u64,
    pub end: u64,
    pub master: u64,
    /// The character's index in select order (Ironclad, Silent, Defect, Regent, Necrobinder),
    /// or -1 for seed `s` playing the `s % 5`th.
    pub character: i32,
    /// 0 to 10.
    pub ascension: u32,
    /// Who plays the fights: 0 the agent, 1 the greedy resolver, 2 the random one.
    pub combat: u32,
    /// 1 to write the combat section's intents.
    pub intents: u32,
    /// The end-to-end options ([`RunOptions`]): 1 to merge the mask into action classes;
    /// player turns a fight may take and agent decisions a run may take before it ends as
    /// capped (0 for no cap); 1 to write the events; 1 to count saturations and unmapped
    /// decisions; 1 to write the privileged buffer ([`sts2_run_envs_priv_ptr`]; `priv_bytes`
    /// per env, for the value head only).
    pub dedupe: u32,
    pub turn_cap: u32,
    pub decision_cap: u32,
    pub events: u32,
    pub counters: u32,
    pub priv_obs: u32,
    /// 1 to write the run observation's v2 section (ABI 8, `sts2_rl::obs_v2`).
    pub obs_v2: u32,
    /// With `obs_v2`, 1 to write the full-view section (ABI 12, `sts2_rl::obs_full`).
    pub obs_full: u32,
    /// 1 to offer the discard-potion actions (ABI 13, `sts2_rl::run_action::discard_offers`).
    pub discard_potion: u32,
    /// The plan column's plays per plan, 1 or 2; 0 is 1 (ABI 13).
    pub plan_depth: u32,
    /// 1 to write the event section (ABI 14, `sts2_rl::obs_events`).
    pub obs_events: u32,
}

/// What the last step of a run batch did ([`RunStepStats`]).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2RunStepStats {
    /// Engine actions; of those, the agent's, the automatic single options and the resolver's.
    pub actions: u64,
    pub decisions: u64,
    pub auto: u64,
    pub resolved: u64,
    /// Listed actions left out of the masks because the engine would refuse them.
    pub refused: u64,
    /// Legal actions the fixed space has no index for, over the masks written.
    pub unmapped: u64,
    /// Episodes that ended; of those, won, capped and stuck.
    pub ended: u32,
    pub won: u32,
    pub capped: u32,
    pub stuck: u32,
    /// Envs reset to a new episode.
    pub reset: u32,
    /// Of the capped episodes, those ended by the fight cap and the decision cap.
    pub fight_capped: u32,
    pub decision_capped: u32,
    /// With `dedupe`: indices merged into an earlier member's class, classes without an index,
    /// decisions taken automatically because they had one class.
    pub merged: u64,
    pub unmapped_classes: u64,
    pub auto_class: u64,
    /// With `counters`: agent decisions with an unmapped legal action, and agent decisions
    /// whose observation saturated each kind of `sts2_rl::SATURATIONS` (any, cards, deck,
    /// creatures, powers, choice, relics, map).
    pub unmapped_decisions: u64,
    pub saturated: [u64; 8],
}

impl From<RunStepStats> for Sts2RunStepStats {
    fn from(s: RunStepStats) -> Self {
        Self {
            actions: s.actions,
            decisions: s.decisions,
            auto: s.auto,
            resolved: s.resolved,
            refused: s.refused,
            unmapped: s.unmapped,
            ended: s.ended,
            won: s.won,
            capped: s.capped,
            stuck: s.stuck,
            reset: s.reset,
            fight_capped: s.fight_capped,
            decision_capped: s.decision_capped,
            merged: s.merged,
            unmapped_classes: s.unmapped_classes,
            auto_class: s.auto_class,
            unmapped_decisions: s.unmapped_decisions,
            saturated: s.saturated,
        }
    }
}

/// The run buffers' layout: sizes per env and the run observation's section offsets
/// (`sts2_rl::run_obs`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2RunLayout {
    pub obs_bytes: u64,
    /// Bytes of the privileged buffer per env ([`sts2_rl::priv_obs`]).
    pub priv_bytes: u64,
    pub mask_words: u64,
    /// Indices in the whole space, and the first run-level one (the combat space comes first).
    pub actions: u64,
    pub run_base: u64,
    pub deck_at: u64,
    pub relics_at: u64,
    pub belt_at: u64,
    pub map_at: u64,
    pub ancient_at: u64,
    pub event_at: u64,
    pub rewards_at: u64,
    pub select_at: u64,
    pub bundles_at: u64,
    pub rest_at: u64,
    pub treasure_at: u64,
    pub shop_at: u64,
    pub combat_at: u64,
    pub combat_static_at: u64,
    /// The v2 section (ABI 8): at `v2_at`, up to `full_at`.
    pub v2_at: u64,
    /// The full-view section (ABI 12): at `full_at`, up to `events_at`.
    pub full_at: u64,
    /// The event section (ABI 14): at `events_at`, `obs_bytes - events_at` bytes.
    pub events_at: u64,
}

/// Writes the run buffers' layout to `out`.
///
/// # Safety
/// `out` is null or valid for writing one [`Sts2RunLayout`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_layout(out: *mut Sts2RunLayout) -> i32 {
    let layout = Sts2RunLayout {
        obs_bytes: RUN_OBS_BYTES as u64,
        priv_bytes: PRIV_BYTES as u64,
        mask_words: RUN_MASK_WORDS as u64,
        actions: RUN_ACTIONS as u64,
        run_base: RUN_BASE as u64,
        deck_at: DECK_AT as u64,
        relics_at: RELICS_AT as u64,
        belt_at: BELT_AT as u64,
        map_at: MAP_AT as u64,
        ancient_at: ANCIENT_AT as u64,
        event_at: EVENT_AT as u64,
        rewards_at: REWARDS_AT as u64,
        select_at: SELECT_AT as u64,
        bundles_at: BUNDLES_AT as u64,
        rest_at: REST_AT as u64,
        treasure_at: TREASURE_AT as u64,
        shop_at: SHOP_AT as u64,
        combat_at: COMBAT_AT as u64,
        combat_static_at: COMBAT_STATIC_AT as u64,
        v2_at: V2_AT as u64,
        full_at: FULL_AT as u64,
        events_at: EVENTS_AT as u64,
    };
    // SAFETY: by the caller's contract `out` is null or writable.
    let Some(out) = (unsafe { out.as_mut() }) else {
        set_error("the layout pointer is null");
        return STS2_ERR_NULL;
    };
    *out = layout;
    STS2_OK
}

/// The run configuration and seeds of `config` (with its seed list copied out of `seeds`).
fn run_config(config: &Sts2RunConfig, seeds: &[u64]) -> Result<(RunConfig, SeedSource), String> {
    let cycle = config.cycle != 0;
    let source = match config.seed_kind {
        0 => SeedSource::List {
            seeds: seeds.to_vec(),
            cycle,
        },
        1 => SeedSource::Range {
            start: config.start,
            end: config.end,
            cycle,
        },
        2 => SeedSource::Random {
            master: config.master,
        },
        other => return Err(format!("unknown seed kind {other}")),
    };
    if source.is_empty() {
        return Err("the seed source has no seed".to_owned());
    }
    let character = match config.character {
        -1 => None,
        i => Some(
            usize::try_from(i)
                .ok()
                .and_then(|i| Character::ALL.get(i).copied())
                .ok_or_else(|| format!("unknown character {i}"))?,
        ),
    };
    let ascension = u8::try_from(config.ascension)
        .ok()
        .filter(|&a| a <= 10)
        .ok_or_else(|| format!("ascension {} is not 0 to 10", config.ascension))?;
    let combat = usize::try_from(config.combat)
        .ok()
        .and_then(|i| CombatMode::ALL.get(i).copied())
        .ok_or_else(|| format!("unknown combat mode {}", config.combat))?;
    let turn_cap = u16::try_from(config.turn_cap)
        .map_err(|_| format!("turn cap {} is over 65,535", config.turn_cap))?;
    let run = RunConfig {
        character,
        ascension,
        combat,
        intents: config.intents != 0,
        options: RunOptions {
            dedupe: config.dedupe != 0,
            turn_cap,
            decision_cap: config.decision_cap,
            events: config.events != 0,
            counters: config.counters != 0,
            priv_obs: config.priv_obs != 0,
            pile_key: false,
            obs_v2: config.obs_v2 != 0,
            plan_column: config.obs_v2 != 0,
            obs_full: config.obs_full != 0,
            discard_potion: config.discard_potion != 0,
            plan_depth: u8::try_from(config.plan_depth)
                .ok()
                .filter(|&d| d <= 2)
                .ok_or_else(|| format!("plan depth {} is not 0 to 2", config.plan_depth))?,
            obs_events: config.obs_events != 0,
        },
    };
    Ok((run, source))
}

/// A batch of `n` run envs stepped on `threads` threads (0 is 1), reset to their first
/// episodes: whole runs from `Run::new` on the seeds `config` gives. Null on error
/// ([`sts2_last_error`]).
///
/// # Safety
/// `config` is null or valid for reading one [`Sts2RunConfig`], whose `seeds` is valid for
/// reading `seeds_len` `u64`s when `seed_kind` is 0 (and may be null if `seeds_len` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_new(
    n: usize,
    config: *const Sts2RunConfig,
    threads: usize,
) -> *mut Sts2RunEnvs {
    // SAFETY: by the caller's contract `config` is null or readable.
    let Some(config) = (unsafe { config.as_ref() }) else {
        set_error("the config pointer is null");
        return std::ptr::null_mut();
    };
    let seeds: &[u64] = if config.seed_kind != 0 || config.seeds.is_null() || config.seeds_len == 0
    {
        &[]
    } else {
        // SAFETY: by the caller's contract a list's `seeds` has `seeds_len` readable `u64`s.
        unsafe { std::slice::from_raw_parts(config.seeds, config.seeds_len) }
    };
    guard(std::ptr::null_mut(), || {
        let (run, source) = match run_config(config, seeds) {
            Ok(parts) => parts,
            Err(message) => {
                set_error(message);
                return std::ptr::null_mut();
            }
        };
        if n == 0 {
            set_error("a batch needs at least one env");
            return std::ptr::null_mut();
        }
        let mut envs = RunEnvs::new(n, source, run);
        envs.threads(threads);
        Box::into_raw(Box::new(Sts2RunEnvs {
            envs,
            stats: Sts2RunStepStats::default(),
            poisoned: false,
        }))
    })
}

/// Frees a run batch (null is a no-op). Its buffers are gone afterwards.
///
/// # Safety
/// `envs` is null or a batch from [`sts2_run_envs_new`] not yet freed and not in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_free(envs: *mut Sts2RunEnvs) {
    if envs.is_null() {
        return;
    }
    // SAFETY: by the caller's contract `envs` came from `Box::into_raw` in `sts2_run_envs_new`
    // and is freed once.
    let envs = unsafe { Box::from_raw(envs) };
    guard((), || drop(envs));
}

/// Applies `actions[i]` (an index of the run space) to run env `i`, for every env still
/// playing.
///
/// Each env plays on to its next decision with more than one option, and the envs whose
/// episode ended (`done`, `won`, `end`, `floor`) are reset. The number of episodes that ended, or a negative `STS2_ERR_*` as [`sts2_envs_step`].
///
/// # Safety
/// `envs` as [`sts2_run_envs_free`]; `actions` is valid for reading `n` aligned `u16`s and is
/// not in one of the batch's buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_step(
    envs: *mut Sts2RunEnvs,
    actions: *const u16,
    n: usize,
) -> i32 {
    if actions.is_null() {
        set_error("the actions pointer is null");
        return STS2_ERR_NULL;
    }
    // SAFETY: by the caller's contract `actions` has `n` readable, aligned `u16`s.
    let actions = unsafe { std::slice::from_raw_parts(actions, n) };
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| match handle.envs.step(actions) {
            Ok(stats) => {
                handle.stats = stats.into();
                i32::try_from(stats.ended).unwrap_or(i32::MAX)
            }
            Err(error) => {
                set_error(error.to_string());
                match error {
                    RunStepError::Length { .. } => STS2_ERR_LENGTH,
                    RunStepError::Illegal { .. } => STS2_ERR_ILLEGAL,
                }
            }
        })
    }
}

/// Starts run env `i` on a fresh run of `seeds[i]` (a learner's resume).
///
/// The runs that end after this take the source's episodes from serial `next` on; see
/// `STS2_RUN_HOLD` for the replay. `STS2_ERR_LENGTH` unless `n` is the batch's length.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `seeds` is valid for reading `n` aligned `u64`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_restart(
    envs: *mut Sts2RunEnvs,
    seeds: *const u64,
    n: usize,
    next: u64,
) -> i32 {
    if seeds.is_null() {
        set_error("the seeds pointer is null");
        return STS2_ERR_NULL;
    }
    // SAFETY: by the caller's contract `seeds` has `n` readable, aligned `u64`s.
    let seeds = unsafe { std::slice::from_raw_parts(seeds, n) };
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            if seeds.len() != handle.envs.len() {
                set_error(format!(
                    "{} seeds for {} envs",
                    seeds.len(),
                    handle.envs.len()
                ));
                return STS2_ERR_LENGTH;
            }
            handle.envs.restart(seeds, next);
            handle.stats = Sts2RunStepStats::default();
            STS2_OK
        })
    }
}

/// Makes run env `dst[i]` a copy of run env `src[i]`'s position, for each of `n` pairs in order
/// (see `RunEnvs::copy_env`).
///
/// With non-null `seeds`, copy `i` is then determinized with `seeds[i]` (draw order and the
/// fight's random streams hidden). `STS2_ERR_LENGTH` if an index is out of range, or a
/// reseeded pair has `src == dst`.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `src` and `dst` are valid for reading `n` aligned `u32`s,
/// and `seeds` is null or valid for reading `n` aligned `u64`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_copy(
    envs: *mut Sts2RunEnvs,
    src: *const u32,
    dst: *const u32,
    seeds: *const u64,
    n: usize,
) -> i32 {
    if src.is_null() || dst.is_null() {
        set_error("a copy pointer is null");
        return STS2_ERR_NULL;
    }
    // SAFETY: by the caller's contract `src` has `n` readable, aligned `u32`s.
    let src = unsafe { std::slice::from_raw_parts(src, n) };
    // SAFETY: by the caller's contract `dst` has `n` readable, aligned `u32`s.
    let dst = unsafe { std::slice::from_raw_parts(dst, n) };
    let seeds = if seeds.is_null() {
        None
    } else {
        // SAFETY: by the caller's contract non-null `seeds` has `n` readable, aligned `u64`s.
        Some(unsafe { std::slice::from_raw_parts(seeds, n) })
    };
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            let len = handle.envs.len();
            if src.iter().chain(dst).any(|&i| i as usize >= len) {
                set_error("a copy index is out of range");
                return STS2_ERR_LENGTH;
            }
            if seeds.is_some() && src.iter().zip(dst).any(|(s, d)| s == d) {
                set_error("a reseeded copy has the same source and destination");
                return STS2_ERR_LENGTH;
            }
            for (i, (&s, &d)) in src.iter().zip(dst).enumerate() {
                let seed = seeds.map(|seeds| seeds[i]);
                handle.envs.copy_env(s as usize, d as usize, seed);
            }
            STS2_OK
        })
    }
}

/// Starts a new episode in every run env.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_reset_all(envs: *mut Sts2RunEnvs) -> i32 {
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            handle.envs.reset_all();
            handle.stats = Sts2RunStepStats::default();
            STS2_OK
        })
    }
}

/// Writes what the last step of a run batch did to `out`.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `out` is null or valid for writing one
/// [`Sts2RunStepStats`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_stats(
    envs: *const Sts2RunEnvs,
    out: *mut Sts2RunStepStats,
) -> i32 {
    // SAFETY: by the caller's contract `out` is null or writable.
    let Some(out) = (unsafe { out.as_mut() }) else {
        set_error("the stats pointer is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract `envs` is null or a live batch nobody writes meanwhile.
    unsafe {
        read_envs(envs, STS2_ERR_NULL, |handle| {
            *out = handle.stats;
            STS2_OK
        })
    }
}

accessor!(
    /// The number of run envs (0 for null).
    Sts2RunEnvs, sts2_run_envs_len -> usize, 0, |h| h.envs.len()
);
accessor!(
    /// The run observations: `len × obs_bytes` bytes ([`Sts2RunLayout`]).
    Sts2RunEnvs, sts2_run_envs_obs_ptr -> *const u8, std::ptr::null(), |h| h.envs.obs().as_ptr()
);
accessor!(
    /// The privileged buffers: `len × priv_bytes` bytes, written only with `priv_obs`, else zero.
    /// For the value head only; a policy must not read them.
    Sts2RunEnvs, sts2_run_envs_priv_ptr -> *const u8, std::ptr::null(),
    |h| h.envs.priv_obs().as_ptr()
);
accessor!(
    /// The observation of the episode that ended in the last step, per env (`len × obs_bytes`);
    /// zero until an episode ends. Use it for terminal HP/floor shaping.
    Sts2RunEnvs, sts2_run_envs_final_obs_ptr -> *const u8, std::ptr::null(),
    |h| h.envs.final_obs().as_ptr()
);
accessor!(
    /// The run masks: `len × mask_words` `u64`s over the run space.
    Sts2RunEnvs, sts2_run_envs_mask_ptr -> *const u64, std::ptr::null(),
    |h| h.envs.masks().as_ptr()
);
accessor!(
    /// Per run env, 1 when its episode ended in the last step.
    Sts2RunEnvs, sts2_run_envs_done_ptr -> *const u8, std::ptr::null(), |h| h.envs.done().as_ptr()
);
accessor!(
    /// Per run env, 1 when its episode ended in the last step with a victory.
    Sts2RunEnvs, sts2_run_envs_won_ptr -> *const u8, std::ptr::null(), |h| h.envs.won().as_ptr()
);
accessor!(
    /// Per run env, how the episode that ended in the last step ended (0 none, 1 died, 2
    /// victory, 3 unported content, 4 stuck, 5 capped, 6 fight capped, 7 decision capped).
    Sts2RunEnvs, sts2_run_envs_end_ptr -> *const u8, std::ptr::null(), |h| h.envs.ends().as_ptr()
);
accessor!(
    /// Per run env, the total floor the episode that ended in the last step reached (`u16`).
    Sts2RunEnvs, sts2_run_envs_floor_ptr -> *const u16, std::ptr::null(),
    |h| h.envs.floors().as_ptr()
);
accessor!(
    /// With `dedupe`, per run env and index (`len × actions` bytes), the member count of the
    /// class each mask index keeps; 0 elsewhere, and all 0 without `dedupe`.
    Sts2RunEnvs, sts2_run_envs_class_counts_ptr -> *const u8, std::ptr::null(),
    |h| h.envs.class_counts().as_ptr()
);
accessor!(
    /// With `events`, per run env, what happened to its episode in the last step.
    ///
    /// Floors entered, bosses killed, a bitmask of the acts whose boss was killed, and 1 for a
    /// won run (`len × 4` bytes, [`RunEvents`]); all 0 without `events`.
    Sts2RunEnvs, sts2_run_envs_events_ptr -> *const RunEvents, std::ptr::null(),
    |h| h.envs.events().as_ptr()
);

accessor!(
    /// The mask of the episode that ended in the last step, per run env.
    ///
    /// `len × mask_words` `u64`s, zero until an episode ends: with `sts2_run_envs_final_obs_ptr`
    /// and `sts2_run_envs_final_priv_ptr`, the position a practice episode's value bootstrap
    /// reads.
    Sts2RunEnvs, sts2_run_envs_final_mask_ptr -> *const u64, std::ptr::null(),
    |h| h.envs.final_masks().as_ptr()
);
accessor!(
    /// With `priv_obs`, the privileged buffer of the episode that ended in the last step.
    ///
    /// Per run env (`len × priv_bytes`); zero otherwise. For the value head only.
    Sts2RunEnvs, sts2_run_envs_final_priv_ptr -> *const u8, std::ptr::null(),
    |h| h.envs.final_priv().as_ptr()
);
accessor!(
    /// Per run env, the cell code of its episode.
    ///
    /// 0 a normal run, else the practice cell's index + 1 (`sts2_rl::practice::CELL_NAMES`).
    Sts2RunEnvs, sts2_run_envs_cell_ptr -> *const u8, std::ptr::null(), |h| h.envs.cells().as_ptr()
);
accessor!(
    /// Per run env, the cell code of the episode that ended in the last step (0 for the others).
    Sts2RunEnvs, sts2_run_envs_ended_cell_ptr -> *const u8, std::ptr::null(),
    |h| h.envs.ended_cells().as_ptr()
);

/// Practice settings (`sts2_rl::PracticeConfig`), by cell index (`sts2_rl::practice::CELL_NAMES`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2PracticeConfig {
    /// Run envs `0..slots` start a practice segment when their episode ends.
    pub slots: u64,
    /// Relative weights of the six cells for one start.
    pub weights: [u32; 6],
    /// Snapshots kept per cell; starts per snapshot (0: no limit); entries a cell needs.
    pub pool: u32,
    pub max_uses: u32,
    pub min_active: u32,
    /// 1: normal runs capture snapshots.
    pub capture: u32,
    /// The seed of the draws and reseeds (read when practice is first set).
    pub seed: u64,
}

/// Practice counts by cell index, and the snapshots each cell holds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2PracticeStats {
    pub pool: [u64; 6],
    pub captured: [u64; 6],
    pub evicted: [u64; 6],
    pub used_up: [u64; 6],
    pub wanted: [u64; 6],
    pub started: [u64; 6],
    pub fell_back: [u64; 6],
    pub to_run: [u64; 6],
    pub bad_start: [u64; 6],
}

/// Turns practice starts on with `config`, or changes their settings; null turns them off
/// (the pool stays). See `sts2_rl::practice`.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `config` is null or valid for reading one
/// [`Sts2PracticeConfig`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_set_practice(
    envs: *mut Sts2RunEnvs,
    config: *const Sts2PracticeConfig,
) -> i32 {
    // SAFETY: by the caller's contract `config` is null or readable.
    let config = unsafe { config.as_ref() }.map(|c| PracticeConfig {
        slots: usize::try_from(c.slots).unwrap_or(usize::MAX),
        weights: c.weights,
        pool: usize::try_from(c.pool).unwrap_or(usize::MAX),
        max_uses: c.max_uses,
        min_active: usize::try_from(c.min_active).unwrap_or(usize::MAX),
        capture: c.capture != 0,
        seed: c.seed,
    });
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            handle.envs.set_practice(config);
            STS2_OK
        })
    }
}

/// Writes the practice counts to `out` (all zero before practice is set).
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `out` is null or valid for writing one
/// [`Sts2PracticeStats`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_practice_stats(
    envs: *const Sts2RunEnvs,
    out: *mut Sts2PracticeStats,
) -> i32 {
    // SAFETY: by the caller's contract `out` is null or writable.
    let Some(out) = (unsafe { out.as_mut() }) else {
        set_error("the stats pointer is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract `envs` is null or a live batch nobody writes meanwhile.
    unsafe {
        read_envs(envs, STS2_ERR_NULL, |handle| {
            *out = handle
                .envs
                .practice()
                .map_or_else(Sts2PracticeStats::default, |pool| {
                    let s = pool.stats();
                    Sts2PracticeStats {
                        pool: pool.sizes().map(|n| u64::try_from(n).unwrap_or(u64::MAX)),
                        captured: s.captured,
                        evicted: s.evicted,
                        used_up: s.used_up,
                        wanted: s.wanted,
                        started: s.started,
                        fell_back: s.fell_back,
                        to_run: s.to_run,
                        bad_start: s.bad_start,
                    }
                });
            STS2_OK
        })
    }
}

/// Gym settings (RLN43, `sts2_rl::GymConfig`), by gym cell index (`sts2_rl::GYM_CELL_NAMES`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2GymConfig {
    /// Run envs `first..first + slots` start a gym fight when their episode ends.
    pub first: u64,
    pub slots: u64,
    pub weights: [u32; GYM_CELLS],
    /// 1: HP 40-100% and one card up or down a level per start.
    pub variants: u32,
    /// A hallway start's HP cap in percent of max HP (0: none).
    pub hall_hp_pct: u32,
    /// 1: the records in order, each once, on seeds `seed + k` (a benchmark).
    pub sequential: u32,
    pub seed: u64,
}

/// Gym counts by gym cell index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2GymStats {
    pub loaded: [u64; GYM_CELLS],
    pub wanted: [u64; GYM_CELLS],
    pub started: [u64; GYM_CELLS],
    pub empty: [u64; GYM_CELLS],
}

/// Loads the gym's starts from a JSON-lines file.
///
/// Records as `sts2_rl::human::HumanStart::from_json`, with no slots until
/// `sts2_run_envs_set_gym`. Returns the number loaded, or an error code (a bad line fails the
/// whole file; `sts2_last_error` names it).
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `path` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_load_gym(
    envs: *mut Sts2RunEnvs,
    path: *const c_char,
) -> i64 {
    if path.is_null() {
        set_error("the gym path is null");
        return i64::from(STS2_ERR_NULL);
    }
    // SAFETY: by the caller's contract a non-null path is NUL-terminated.
    let path = unsafe { CStr::from_ptr(path) };
    let starts = path
        .to_str()
        .map_err(|_| "the gym path is not UTF-8".to_owned())
        .and_then(|p| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}")))
        .and_then(|text| Gym::parse(&text));
    let starts = match starts {
        Ok(s) => s,
        Err(e) => {
            set_error(e);
            return i64::from(STS2_ERR_ARGUMENT);
        }
    };
    let mut loaded = 0;
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    let code = unsafe {
        with_envs(envs, |handle| {
            loaded = handle.envs.load_gym(starts);
            STS2_OK
        })
    };
    if code == STS2_OK {
        i64::try_from(loaded).unwrap_or(i64::MAX)
    } else {
        i64::from(code)
    }
}

/// New gym settings (after `sts2_run_envs_load_gym`; ignored before); null gives it no slots.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `config` is null or valid for reading one
/// [`Sts2GymConfig`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_set_gym(
    envs: *mut Sts2RunEnvs,
    config: *const Sts2GymConfig,
) -> i32 {
    // SAFETY: by the caller's contract `config` is null or readable.
    let config = unsafe { config.as_ref() }.map(|c| GymConfig {
        first: usize::try_from(c.first).unwrap_or(usize::MAX),
        slots: usize::try_from(c.slots).unwrap_or(usize::MAX),
        weights: c.weights,
        variants: c.variants != 0,
        hall_hp_pct: c.hall_hp_pct,
        sequential: c.sequential != 0,
        seed: c.seed,
    });
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    unsafe {
        with_envs(envs, |handle| {
            handle.envs.set_gym(config);
            STS2_OK
        })
    }
}

/// Writes the gym counts to `out` (all zero before a gym is loaded).
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `out` is null or valid for writing one
/// [`Sts2GymStats`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_gym_stats(
    envs: *const Sts2RunEnvs,
    out: *mut Sts2GymStats,
) -> i32 {
    // SAFETY: by the caller's contract `out` is null or writable.
    let Some(out) = (unsafe { out.as_mut() }) else {
        set_error("the stats pointer is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract `envs` is null or a live batch nobody writes meanwhile.
    unsafe {
        read_envs(envs, STS2_ERR_NULL, |handle| {
            *out = handle.envs.gym().map_or_else(Sts2GymStats::default, |g| {
                let s = g.stats();
                Sts2GymStats {
                    loaded: s.loaded,
                    wanted: s.wanted,
                    started: s.started,
                    empty: s.empty,
                }
            });
            STS2_OK
        })
    }
}

/// Writes env `i`'s fight start record into `buf`, NUL-terminated when it fits (NAZ1).
///
/// The record is `sts2_rl::human::start_record`'s JSON line. Returns its length in bytes (more
/// than `cap - 1`: `buf` was too small and holds nothing useful), 0 when the env is not in a
/// combat room, or `STS2_ERR_LENGTH` for an index out of range. An optional symbol, so the ABI
/// version is unchanged.
///
/// # Safety
/// As [`sts2_run_envs_step`]'s `envs`; `buf` is null or valid for writing `cap` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_envs_start_record(
    envs: *mut Sts2RunEnvs,
    i: usize,
    buf: *mut u8,
    cap: usize,
) -> i64 {
    let mut line = None;
    // SAFETY: by the caller's contract `envs` is null or a live batch used by no other call.
    let code = unsafe {
        with_envs(envs, |handle| {
            if i >= handle.envs.len() {
                set_error("the env index is out of range");
                return STS2_ERR_LENGTH;
            }
            line = handle.envs.run(i).and_then(sts2_rl::human::start_record);
            STS2_OK
        })
    };
    if code != STS2_OK {
        return i64::from(code);
    }
    let Some(line) = line else { return 0 };
    let bytes = line.as_bytes();
    if !buf.is_null() && bytes.len() < cap {
        // SAFETY: `buf` has `cap > bytes.len()` writable bytes by the caller's contract.
        let dst = unsafe { std::slice::from_raw_parts_mut(buf, bytes.len() + 1) };
        dst[..bytes.len()].copy_from_slice(bytes);
        dst[bytes.len()] = 0;
    }
    i64::try_from(bytes.len()).unwrap_or(i64::MAX)
}

/// Whether one gym record parses and its fight sets up on `seed`.
///
/// `STS2_OK`, else `STS2_ERR_ARGUMENT` with the reason in `sts2_last_error` (its first word the
/// kind: `card`, `relic`, `potion`, `encounter`, `field`, `act`, or a set-up step).
///
/// # Safety
/// `line` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_human_check(line: *const c_char, seed: u64) -> i32 {
    if line.is_null() {
        set_error("the line is null");
        return STS2_ERR_NULL;
    }
    // SAFETY: by the caller's contract `line` is NUL-terminated.
    let line = unsafe { CStr::from_ptr(line) };
    guard(STS2_ERR_PANIC, || {
        let result = line
            .to_str()
            .map_err(|_| "field utf-8".to_owned())
            .and_then(HumanStart::from_json)
            .and_then(|s| {
                s.build(seed, sts2_rl::human::Variant::default(), 0)
                    .map(drop)
            });
        match result {
            Ok(()) => STS2_OK,
            Err(e) => {
                set_error(e);
                STS2_ERR_ARGUMENT
            }
        }
    })
}

/// The static action → entity table (`sts2_rl::entity`), valid for the library's lifetime.
///
/// `actions × 4` bytes: per index of the run space its entity kind, slot, sub-slot (a target or
/// a reward's card) and phase (255 for any).
#[unsafe(no_mangle)]
pub extern "C" fn sts2_run_action_table() -> *const u8 {
    sts2_rl::entity::ACTION_ENTITIES.as_ptr().cast()
}

/// The content-stats table `table`, valid for the library's lifetime.
///
/// `table` is 0 cards, 1 cards upgraded, 2 relics or 3 potions (`sts2_rl::content_stats`): `rows
/// x sts2_rl::content_stats::WIDTH` `int16` by vocab id; the row count goes to `rows`. Null for
/// another `table` or a null `rows`.
///
/// # Safety
/// `rows` is null or valid for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_content_stats(table: u32, rows: *mut usize) -> *const i16 {
    guard(std::ptr::null(), || {
        let Some(t) = content_stats::Table::from_index(table) else {
            set_error(format!("no content-stats table {table}"));
            return std::ptr::null();
        };
        if rows.is_null() {
            set_error("null rows");
            return std::ptr::null();
        }
        let flat = content_stats::flat(t);
        // SAFETY: the caller passes a valid `rows` (checked non-null).
        unsafe { rows.write(flat.len() / content_stats::WIDTH) };
        flat.as_ptr()
    })
}

/// The content-stats columns as one NUL-terminated string, `name:m` (a magnitude) or `name:f`
/// (a flag or a -1/0/1 delta) joined by commas, in column order; static.
#[unsafe(no_mangle)]
pub extern "C" fn sts2_content_stats_columns() -> *const c_char {
    static COLUMNS: OnceLock<CString> = OnceLock::new();
    guard(std::ptr::null(), || {
        COLUMNS
            .get_or_init(|| {
                let text = content_stats::columns()
                    .iter()
                    .map(|c| format!("{}:{}", c.name, if c.magnitude { 'm' } else { 'f' }))
                    .collect::<Vec<_>>()
                    .join(",");
                CString::new(text).unwrap_or_default()
            })
            .as_ptr()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_stats_are_exported() {
        let mut rows = 0;
        // SAFETY: `rows` is a valid local.
        let p = unsafe { sts2_content_stats(0, &raw mut rows) };
        assert!(!p.is_null());
        assert_eq!(rows, vocab::CARDS.ids().count());
        // SAFETY: as above.
        assert!(unsafe { sts2_content_stats(4, &raw mut rows) }.is_null());
        // SAFETY: a static NUL-terminated string.
        let cols = unsafe { CStr::from_ptr(sts2_content_stats_columns()) };
        let n = cols.to_str().map(|s| s.split(',').count());
        assert_eq!(n, Ok(content_stats::WIDTH));
    }

    #[test]
    fn a_panic_poisons_the_batch_and_is_reported() {
        let envs = new_envs(2, Ok(default_source()), 7, 1);
        assert!(!envs.is_null());
        // SAFETY: `envs` is the live batch just made, used by nothing else.
        let code = unsafe { with_envs(envs, |_| panic!("injected")) };
        assert_eq!(code, STS2_ERR_PANIC);
        // SAFETY: the last error is a NUL-terminated string owned by this thread.
        let message = unsafe { CStr::from_ptr(sts2_last_error()) };
        assert_eq!(message.to_str(), Ok("panic: injected"));
        // SAFETY: as above.
        assert_eq!(unsafe { sts2_envs_reset_all(envs) }, STS2_ERR_POISONED);
        // SAFETY: `envs` is freed once.
        unsafe { sts2_envs_free(envs) };
    }

    #[test]
    fn guard_returns_the_fallback_on_a_panic() {
        assert_eq!(guard(-1, || 3), 3);
        assert_eq!(guard(-1, || panic!("no")), -1);
    }
}

// ---------------------------------------------------------------------------------------------
// CB2: the combat search (`sts2_rl::Search`, `python/sts2/search.py`). A search belongs to the
// run batch it was made for; one call at a time on either.

/// [`sts2_rl::SearchConfig`] as C sees it. `world`: 0 fresh, 1 paired, 2 one, 3 exact.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Sts2SearchConfig {
    pub sims: u32,
    pub m: u32,
    pub wave: u32,
    pub depth: u32,
    pub threads: u32,
    pub world: u32,
    pub keep_tree: u32,
    pub reserved: u32,
    pub c_puct: f64,
    pub c_visit: f64,
    pub c_scale: f64,
    pub hp_coef: f64,
    pub pot_w: f64,
}

/// A search over a run batch's fights.
#[derive(Debug)]
pub struct Sts2Search {
    search: sts2_rl::Search,
    poisoned: bool,
}

/// Runs `f` on a live search (and its batch), catching panics (they poison the search).
///
/// # Safety
/// `search` is null or a live search, `envs` null or a live batch, neither used by another call.
unsafe fn with_search(
    search: *mut Sts2Search,
    envs: *mut Sts2RunEnvs,
    f: impl FnOnce(&mut sts2_rl::Search, Option<&RunEnvs>) -> i32,
) -> i32 {
    // SAFETY: by the caller's contract `search` is null or live and unaliased.
    let Some(s) = (unsafe { search.as_mut() }) else {
        set_error("the search pointer is null");
        return STS2_ERR_NULL;
    };
    if s.poisoned {
        set_error("the search panicked in an earlier call");
        return STS2_ERR_POISONED;
    }
    // SAFETY: by the caller's contract `envs` is null or live and unaliased.
    let envs = unsafe { envs.as_ref() }.map(|h| &h.envs);
    match panic::catch_unwind(AssertUnwindSafe(|| f(&mut s.search, envs))) {
        Ok(code) => code,
        Err(payload) => {
            s.poisoned = true;
            set_error(format!("panic: {}", panic_message(&*payload)));
            STS2_ERR_PANIC
        }
    }
}

/// A slice of `n` `T`s at `p`, or an error if `p` is null.
///
/// # Safety
/// Non-null `p` is valid for reading `n` aligned `T`s for the returned lifetime.
unsafe fn slice_in<'a, T>(p: *const T, n: usize) -> Option<&'a [T]> {
    // SAFETY: by the caller's contract.
    (!p.is_null()).then(|| unsafe { std::slice::from_raw_parts(p, n) })
}

/// A mutable slice of `n` `T`s at `p`, or `None` if `p` is null.
///
/// # Safety
/// Non-null `p` is valid for writing `n` aligned `T`s, unaliased, for the returned lifetime.
unsafe fn slice_out<'a, T>(p: *mut T, n: usize) -> Option<&'a mut [T]> {
    // SAFETY: by the caller's contract.
    (!p.is_null()).then(|| unsafe { std::slice::from_raw_parts_mut(p, n) })
}

/// A new search over run batch `envs`; null on error. Free it with [`sts2_search_free`].
///
/// # Safety
/// `envs` is null or a live batch; `config` null or valid for reading one config.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_new(
    envs: *mut Sts2RunEnvs,
    config: *const Sts2SearchConfig,
) -> *mut Sts2Search {
    // SAFETY: by the caller's contract.
    let (Some(h), Some(c)) = (unsafe { envs.as_ref() }, unsafe { config.as_ref() }) else {
        set_error("a null pointer");
        return std::ptr::null_mut();
    };
    let world = match c.world {
        0 => sts2_rl::World::Fresh,
        1 => sts2_rl::World::Paired,
        2 => sts2_rl::World::One,
        3 => sts2_rl::World::Exact,
        _ => {
            set_error("world is 0-3");
            return std::ptr::null_mut();
        }
    };
    let cfg = sts2_rl::SearchConfig {
        sims: c.sims as usize,
        m: c.m as usize,
        wave: c.wave.max(1) as usize,
        c_puct: c.c_puct,
        c_visit: c.c_visit,
        c_scale: c.c_scale,
        depth: c.depth as usize,
        hp_coef: c.hp_coef,
        pot_w: c.pot_w,
        world,
        keep_tree: c.keep_tree != 0,
        threads: c.threads.max(1) as usize,
    };
    Box::into_raw(Box::new(Sts2Search {
        search: sts2_rl::Search::new(cfg, &h.envs),
        poisoned: false,
    }))
}

/// Frees a search (null is a no-op).
///
/// # Safety
/// `search` is null or a search from [`sts2_search_new`] not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_free(search: *mut Sts2Search) {
    if !search.is_null() {
        // SAFETY: by the caller's contract it came from `Box::into_raw` and is freed once.
        drop(unsafe { Box::from_raw(search) });
    }
}

/// Starts a decision (`sts2_rl::Search::begin`): `n` main envs, `width` logits and priors per
/// row (f64), Gumbel noise (`width` per row, or null), `per_row` world seeds per row.
///
/// # Safety
/// As [`with_search`]; each array is valid for reading its stated count.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments, reason = "flat C arrays")]
pub unsafe extern "C" fn sts2_search_begin(
    search: *mut Sts2Search,
    envs: *mut Sts2RunEnvs,
    mains: *const u32,
    n: usize,
    width: usize,
    logits: *const f64,
    prior: *const f64,
    gumbel: *const f64,
    seeds: *const u64,
    per_row: usize,
) -> i32 {
    // SAFETY: by the caller's contract the array has its stated count.
    let mains = unsafe { slice_in(mains, n) };
    // SAFETY: by the caller's contract the array has its stated count.
    let logits = unsafe { slice_in(logits, n * width) };
    // SAFETY: by the caller's contract the array has its stated count.
    let prior = unsafe { slice_in(prior, n * width) };
    // SAFETY: by the caller's contract the array has its stated count.
    let seeds = unsafe { slice_in(seeds, n * per_row) };
    // SAFETY: as above; null means no noise.
    let gumbel = unsafe { slice_in(gumbel, n * width) }.unwrap_or(&[]);
    let (Some(mains), Some(logits), Some(prior), Some(seeds)) = (mains, logits, prior, seeds)
    else {
        set_error("a begin array is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract.
    unsafe {
        with_search(search, envs, |s, envs| {
            let Some(envs) = envs else {
                set_error("the envs pointer is null");
                return STS2_ERR_NULL;
            };
            if per_row == 0 || mains.iter().any(|&m| m as usize >= envs.len()) {
                set_error("a main env is out of range, or no seeds");
                return STS2_ERR_LENGTH;
            }
            let mains: Vec<usize> = mains.iter().map(|&m| m as usize).collect();
            s.begin(envs, &mains, width, logits, prior, gumbel, seeds, per_row);
            STS2_OK
        })
    }
}

/// Advances the search to its next leaves (`sts2_rl::Search::run`), written to `obs`, `masks`
/// and `privs` (room for `cap` rows each); the number of rows (0: every fight has its move).
///
/// # Safety
/// As [`with_search`]; the buffers are valid for writing `cap` rows.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_run(
    search: *mut Sts2Search,
    obs: *mut u8,
    masks: *mut u64,
    privs: *mut u8,
    cap: usize,
) -> i32 {
    // SAFETY: by the caller's contract the array has its stated count.
    let obs = unsafe { slice_out(obs, cap * RUN_OBS_BYTES) };
    // SAFETY: by the caller's contract the array has its stated count.
    let masks = unsafe { slice_out(masks, cap * RUN_MASK_WORDS) };
    // SAFETY: by the caller's contract the array has its stated count.
    let privs = unsafe { slice_out(privs, cap * PRIV_BYTES) };
    let bufs = (obs, masks, privs);
    let (Some(obs), Some(masks), Some(privs)) = bufs else {
        set_error("a request buffer is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract.
    unsafe {
        with_search(search, std::ptr::null_mut(), |s, _| {
            let rows = s.run(&mut sts2_rl::Requests { obs, masks, privs });
            i32::try_from(rows).unwrap_or(STS2_ERR_LENGTH)
        })
    }
}

/// The net's outputs for the last run's `rows` rows (`sts2_rl::Search::feed`).
///
/// # Safety
/// As [`with_search`]; `logits` and `prior` hold `rows * width` f64s, `values` `rows`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_feed(
    search: *mut Sts2Search,
    logits: *const f64,
    prior: *const f64,
    values: *const f64,
    rows: usize,
    width: usize,
) -> i32 {
    // SAFETY: by the caller's contract the array has its stated count.
    let logits = unsafe { slice_in(logits, rows * width) };
    // SAFETY: by the caller's contract the array has its stated count.
    let prior = unsafe { slice_in(prior, rows * width) };
    // SAFETY: by the caller's contract the array has its stated count.
    let values = unsafe { slice_in(values, rows) };
    let arrays = (logits, prior, values);
    let (Some(logits), Some(prior), Some(values)) = arrays else {
        set_error("a feed array is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract.
    unsafe {
        with_search(search, std::ptr::null_mut(), |s, _| {
            s.feed(logits, prior, values);
            STS2_OK
        })
    }
}

/// Per row of the decision: the move, the root's visits and the top `top` of the improved
/// policy (actions, -1 past the legal ones, and probabilities).
///
/// # Safety
/// As [`with_search`]; `actions` and `visits` hold `n` values, `top_a` and `top_p` `n * top`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_results(
    search: *mut Sts2Search,
    actions: *mut u16,
    visits: *mut f64,
    top_a: *mut i16,
    top_p: *mut f32,
    top: usize,
    n: usize,
) -> i32 {
    // SAFETY: by the caller's contract the array has its stated count.
    let actions = unsafe { slice_out(actions, n) };
    // SAFETY: by the caller's contract the array has its stated count.
    let visits = unsafe { slice_out(visits, n) };
    // SAFETY: by the caller's contract the array has its stated count.
    let top_a = unsafe { slice_out(top_a, n * top) };
    // SAFETY: by the caller's contract the array has its stated count.
    let top_p = unsafe { slice_out(top_p, n * top) };
    let bufs = (actions, visits, top_a, top_p);
    let (Some(actions), Some(visits), Some(top_a), Some(top_p)) = bufs else {
        set_error("a results buffer is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract.
    unsafe {
        with_search(search, std::ptr::null_mut(), |s, _| {
            let res = s.results();
            if res.len() != n {
                set_error(format!("{} rows, the decision has {}", n, res.len()));
                return STS2_ERR_LENGTH;
            }
            for (r, x) in res.iter().enumerate() {
                actions[r] = x.action;
                visits[r] = x.visits;
                for k in 0..top {
                    let (a, p) = x.policy.get(k).map_or((-1, 0.0), |&(a, p)| {
                        (i16::try_from(a).unwrap_or(-1), {
                            #[allow(
                                clippy::cast_possible_truncation,
                                reason = "a probability stored as f32"
                            )]
                            let p = p as f32;
                            p
                        })
                    });
                    top_a[r * top + k] = a;
                    top_p[r * top + k] = p;
                }
            }
            STS2_OK
        })
    }
}

/// After the real step of main envs `mains` (`sts2_rl::Search::after_step`).
///
/// # Safety
/// As [`with_search`]; `mains` holds `n` u32s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_after_step(
    search: *mut Sts2Search,
    envs: *mut Sts2RunEnvs,
    mains: *const u32,
    n: usize,
) -> i32 {
    // SAFETY: by the caller's contract.
    let Some(mains) = (unsafe { slice_in(mains, n) }) else {
        set_error("mains is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract.
    unsafe {
        with_search(search, envs, |s, envs| {
            let Some(envs) = envs else {
                set_error("the envs pointer is null");
                return STS2_ERR_NULL;
            };
            let mains: Vec<usize> = mains.iter().map(|&m| m as usize).collect();
            s.after_step(envs, &mains);
            STS2_OK
        })
    }
}

/// The search's counters (`sts2_rl::SearchStats::to_array`, 10 u64s).
///
/// # Safety
/// As [`with_search`]; `out` is valid for writing 10 u64s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_search_stats(search: *mut Sts2Search, out: *mut u64) -> i32 {
    // SAFETY: by the caller's contract.
    let Some(out) = (unsafe { slice_out(out, 10) }) else {
        set_error("out is null");
        return STS2_ERR_NULL;
    };
    // SAFETY: by the caller's contract.
    unsafe {
        with_search(search, std::ptr::null_mut(), |s, _| {
            out.copy_from_slice(&s.stats().to_array());
            STS2_OK
        })
    }
}
