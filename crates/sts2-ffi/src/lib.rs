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
use sts2_rl::run_action::RUN_BASE;
use sts2_rl::run_obs::{
    ANCIENT_AT, BELT_AT, BUNDLES_AT, COMBAT_AT, COMBAT_STATIC_AT, DECK_AT, EVENT_AT, MAP_AT,
    RELICS_AT, REST_AT, REWARDS_AT, SELECT_AT, SHOP_AT, TREASURE_AT,
};
use sts2_rl::{
    ACTIONS, CombatMode, Envs, MASK_WORDS, OBS_BYTES, Order, RUN_ACTIONS, RUN_MASK_WORDS,
    RUN_OBS_BYTES, RunConfig, RunEnvs, RunStepError, RunStepStats, STATIC_BYTES, SeedSource, Start,
    StartSource, StepError, StepStats, vocab,
};

/// The version of this interface; it changes with any signature, layout or meaning.
pub const STS2_ABI_VERSION: u32 = 3;

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
        }
    }
}

/// The run buffers' layout: sizes per env and the run observation's section offsets
/// (`sts2_rl::run_obs`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sts2RunLayout {
    pub obs_bytes: u64,
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
}

/// Writes the run buffers' layout to `out`.
///
/// # Safety
/// `out` is null or valid for writing one [`Sts2RunLayout`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sts2_run_layout(out: *mut Sts2RunLayout) -> i32 {
    let layout = Sts2RunLayout {
        obs_bytes: RUN_OBS_BYTES as u64,
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
    let run = RunConfig {
        character,
        ascension,
        combat,
        intents: config.intents != 0,
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
    /// victory, 3 unported content, 4 stuck, 5 capped).
    Sts2RunEnvs, sts2_run_envs_end_ptr -> *const u8, std::ptr::null(), |h| h.envs.ends().as_ptr()
);
accessor!(
    /// Per run env, the total floor the episode that ended in the last step reached (`u16`).
    Sts2RunEnvs, sts2_run_envs_floor_ptr -> *const u16, std::ptr::null(),
    |h| h.envs.floors().as_ptr()
);

#[cfg(test)]
mod tests {
    use super::*;

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
