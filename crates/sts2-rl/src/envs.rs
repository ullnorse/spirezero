//! The batch of environments a vectorised trainer steps (`docs/specs/rl-2x-plan.md`,
//! section 7.4).
//!
//! [`Envs`] owns `n` fights and flat, stable buffers: the observations (`n` × [`OBS_BYTES`]),
//! the static observations (`n` × [`STATIC_BYTES`], rewritten with the observation, so the belt
//! is the current one), the masks (`n` ×
//! [`MASK_WORDS`] words) and the `done`/`won` bytes of the last step. They are allocated by
//! [`Envs::new`] and never resized, so they keep their address for the batch's life (the C ABI,
//! `sts2-ffi`, hands them out once).
//!
//! [`Envs::step`] applies one action index per env, ends the episodes that are over (won, lost,
//! stopped at unported content, out of mapped actions, or at [`MAX_EPISODE_ACTIONS`]), resets those
//! envs from the source's next episodes (auto-reset: their observation is the new episode's first)
//! and writes every live env's mask and observation. Episodes are handed out in env order after the
//! envs have stepped, so which env plays which episode, and every trajectory, is the same for any
//! [`Envs::threads`]. Rewards are the trainer's: `done`, `won`, HP and block are in the buffers.

use std::thread;

use sts2_core::combat::Action;
use sts2_core::{Combat, Status};

use crate::action::{MASK_WORDS, Mask, index_of};
use crate::obs::{OBS_BYTES, Observer, STATIC_BYTES, observe_static};
use crate::start::{Episode, StartSource};

/// Actions an episode may take before it ends (random play ends far sooner).
pub const MAX_EPISODE_ACTIONS: u64 = 10_000;

/// A finished episode, as [`Envs::step_with`] reports it before the env resets.
#[derive(Clone, Copy, Debug)]
pub struct EpisodeEnd<'a> {
    pub env: usize,
    pub episode: Episode,
    /// The seed of the episode's [`Start`](crate::Start).
    pub seed: u64,
    /// The fight's status at the end ([`Status::PlayerTurn`] or [`Status::Decision`] when it
    /// hit the action cap or ran out of mapped actions).
    pub status: Status,
    pub actions: u64,
    /// The final position.
    pub combat: &'a Combat,
}

impl EpisodeEnd<'_> {
    /// Whether the episode was won.
    #[must_use]
    pub fn won(&self) -> bool {
        self.status == Status::Won
    }
}

/// What one [`Envs::step`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepStats {
    /// Actions applied.
    pub actions: u64,
    /// Episodes that ended, and of those, won and capped at [`MAX_EPISODE_ACTIONS`].
    pub ended: u32,
    pub won: u32,
    pub capped: u32,
    /// Envs reset to a new episode.
    pub reset: u32,
    /// Legal actions the fixed space has no index for, over the masks written.
    pub unmapped: u64,
}

/// Why [`Envs::step`] refused a batch; nothing was applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepError {
    /// The batch does not have one action per env.
    Length { expected: usize, got: usize },
    /// The action of a live env is not set in its mask.
    Illegal { env: usize, action: u16 },
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Length { expected, got } => {
                write!(f, "expected {expected} actions, got {got}")
            }
            Self::Illegal { env, action } => write!(f, "action {action} is not legal in env {env}"),
        }
    }
}

impl std::error::Error for StepError {}

/// One environment.
#[derive(Debug)]
struct Env {
    combat: Combat,
    observer: Observer,
    legal: Vec<Action>,
    mask: Mask,
    /// The episode being played; `None` once the source has no more.
    episode: Option<Episode>,
    actions: u64,
    /// The episode ended in this step.
    ended: bool,
    /// A new episode was assigned in this step.
    fresh: bool,
}

/// Whether `combat` takes actions.
const fn live(combat: &Combat) -> bool {
    matches!(combat.status(), Status::PlayerTurn | Status::Decision)
}

/// What the envs of one pass did, summed.
#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    actions: u64,
    unmapped: u64,
}

impl std::ops::Add for Counts {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            actions: self.actions + other.actions,
            unmapped: self.unmapped + other.unmapped,
        }
    }
}

/// One env's slices of the batch buffers.
struct Slots<'a> {
    obs: &'a mut [u8],
    statics: &'a mut [u8],
    mask: &'a mut [u64],
}

impl Env {
    /// Writes the env's mask and (with `observe`) observation; the unmapped count. A live
    /// fight with an empty mask ends.
    fn extract(&mut self, slots: &mut Slots<'_>, observe: bool) -> u64 {
        self.combat.legal_actions_into(&mut self.legal);
        let unmapped = self.mask.fill(&self.combat, &self.legal);
        debug_assert_eq!(
            check_mask(&self.combat, &self.legal, &self.mask),
            Ok(()),
            "the mask disagrees with the legal list"
        );
        slots.mask.copy_from_slice(self.mask.bits());
        if observe {
            self.observer.observe(&self.combat, slots.obs);
            observe_static(&self.combat, slots.statics);
        }
        if self.mask.count() == 0 {
            self.ended = true;
        }
        u64::from(unmapped)
    }

    /// Applies `action` (nothing if the fight is already over or the episode has ended), then
    /// writes the mask and observation unless the episode ended.
    fn step(&mut self, action: u16, slots: &mut Slots<'_>, observe: bool) -> Counts {
        let mut counts = Counts::default();
        if self.episode.is_none() || self.ended {
            return counts;
        }
        if live(&self.combat) {
            let position = self
                .mask
                .position(usize::from(action))
                .expect("the batch was checked against the masks");
            let action = self.legal.swap_remove(position);
            self.combat.apply(action).expect("a listed action applies");
            self.actions += 1;
            counts.actions = 1;
        }
        if !live(&self.combat) || self.actions >= MAX_EPISODE_ACTIONS {
            self.ended = true;
        } else {
            counts.unmapped = self.extract(slots, observe);
        }
        counts
    }

    /// Resets to the fresh episode's start and writes everything.
    fn reset(
        &mut self,
        source: &StartSource,
        slots: &mut Slots<'_>,
        observe: bool,
        blindfold: Option<u64>,
    ) -> Counts {
        self.fresh = false;
        self.ended = false;
        let Some(episode) = self.episode else {
            slots.mask.fill(0);
            return Counts::default();
        };
        let start = &source.starts()[episode.start].combat;
        match blindfold {
            Some(seed) => self.combat = start.blindfold_for_search(mix(seed ^ episode.serial)),
            None => self.combat.clone_from(start),
        }
        self.actions = 0;
        self.observer.reset();
        Counts {
            actions: 0,
            unmapped: self.extract(slots, observe),
        }
    }
}

/// Checks that `mask` is `legal`'s: every set bit decodes to the first listed action with
/// that index, every listed action the space maps has its index's bit, and nothing else is set.
///
/// # Errors
/// The first disagreement.
pub fn check_mask(combat: &Combat, legal: &[Action], mask: &Mask) -> Result<(), String> {
    let mut mapped = Mask::new();
    for (i, action) in legal.iter().enumerate() {
        let Some(index) = index_of(combat, action) else {
            continue;
        };
        if mapped.contains(index) {
            continue;
        }
        mapped.fill_one(index, i);
        if mask.position(index) != Some(i) {
            return Err(format!("{action:?} (index {index}) is not at position {i}"));
        }
    }
    if mask.bits() != mapped.bits() {
        return Err(format!(
            "{} bits set for {} mapped indices",
            mask.count(),
            mapped.count()
        ));
    }
    Ok(())
}

/// Runs `f` on every env with its index and slots, on up to `threads` scoped threads (each
/// owning a contiguous share of the envs); the sum of its results.
fn each_env<F>(
    threads: usize,
    envs: &mut [Env],
    obs: &mut [u8],
    statics: &mut [u8],
    masks: &mut [u64],
    f: F,
) -> Counts
where
    F: Fn(usize, &mut Env, &mut Slots<'_>) -> Counts + Sync,
{
    let run =
        |first: usize, envs: &mut [Env], obs: &mut [u8], statics: &mut [u8], masks: &mut [u64]| {
            let shares = obs
                .as_chunks_mut::<OBS_BYTES>()
                .0
                .iter_mut()
                .zip(statics.as_chunks_mut::<STATIC_BYTES>().0)
                .zip(masks.as_chunks_mut::<MASK_WORDS>().0);
            envs.iter_mut()
                .zip(shares)
                .enumerate()
                .map(|(i, (env, ((obs, statics), mask)))| {
                    f(first + i, env, &mut Slots { obs, statics, mask })
                })
                .fold(Counts::default(), |sum, c| sum + c)
        };
    if threads <= 1 || envs.len() < 2 {
        return run(0, envs, obs, statics, masks);
    }
    let per = envs.len().div_ceil(threads);
    thread::scope(|scope| {
        let handles: Vec<_> = envs
            .chunks_mut(per)
            .zip(obs.chunks_mut(per * OBS_BYTES))
            .zip(statics.chunks_mut(per * STATIC_BYTES))
            .zip(masks.chunks_mut(per * MASK_WORDS))
            .enumerate()
            .map(|(t, (((envs, obs), statics), masks))| {
                let run = &run;
                scope.spawn(move || run(t * per, envs, obs, statics, masks))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("an env thread panicked"))
            .fold(Counts::default(), |sum, c| sum + c)
    })
}

/// A batch of environments.
#[derive(Debug)]
pub struct Envs {
    batch: Vec<Env>,
    source: StartSource,
    seed: u64,
    /// The next episode's serial.
    next: u64,
    threads: usize,
    observe: bool,
    obs: Vec<u8>,
    statics: Vec<u8>,
    masks: Vec<u64>,
    done: Vec<u8>,
    won: Vec<u8>,
    final_hp: Vec<i16>,
    /// Episodes start from their start with the hidden part resampled under this seed.
    blindfold: Option<u64>,
}

/// `SplitMix64` of `x`.
const fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl Envs {
    /// `n` envs over `source` (episodes in its order; `seed` drives [`Order::Random`]), reset
    /// to their first episodes, writing masks and full observations.
    ///
    /// # Panics
    /// If `source` has no start.
    ///
    /// [`Order::Random`]: crate::start::Order::Random
    #[must_use]
    pub fn new(n: usize, source: StartSource, seed: u64) -> Self {
        Self::with_observation(n, source, seed, true, true)
    }

    /// [`Envs::new`], with steps writing the observation only if `observe` (the mask always),
    /// and its intents section only if `intents`.
    ///
    /// # Panics
    /// If `source` has no start.
    #[must_use]
    pub fn with_observation(
        n: usize,
        source: StartSource,
        seed: u64,
        observe: bool,
        intents: bool,
    ) -> Self {
        let any = &source
            .starts()
            .first()
            .expect("a source with starts")
            .combat;
        let envs = (0..n)
            .map(|_| Env {
                combat: any.clone(),
                observer: Observer::new(intents),
                legal: Vec::new(),
                mask: Mask::new(),
                episode: None,
                actions: 0,
                ended: false,
                fresh: false,
            })
            .collect();
        let mut envs = Self {
            batch: envs,
            source,
            seed,
            next: 0,
            threads: 1,
            observe,
            obs: vec![0; n * OBS_BYTES],
            statics: vec![0; n * STATIC_BYTES],
            masks: vec![0; n * MASK_WORDS],
            done: vec![0; n],
            won: vec![0; n],
            final_hp: vec![0; n],
            blindfold: None,
        };
        envs.reset_all();
        envs
    }

    /// The number of envs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.batch.len()
    }

    /// Whether there are no envs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Steps the envs on up to `t` threads (at least 1), each owning a contiguous share.
    pub fn threads(&mut self, t: usize) {
        self.threads = t.max(1);
    }

    /// From the next reset on, each episode starts from its start with what the player cannot
    /// see (the draw order, the RNG streams) resampled from `seed` and the episode's serial
    /// ([`Combat::blindfold_for_search`]); `None` starts from the start itself.
    pub const fn blindfold(&mut self, seed: Option<u64>) {
        self.blindfold = seed;
    }

    /// Starts a new episode in every env (the next ones of the source, in env order).
    pub fn reset_all(&mut self) {
        for env in &mut self.batch {
            env.episode = self.source.episode(self.next, self.seed);
            self.next += 1;
            env.fresh = true;
        }
        self.done.fill(0);
        self.won.fill(0);
        self.final_hp.fill(0);
        let (source, observe, blindfold) = (&self.source, self.observe, self.blindfold);
        each_env(
            self.threads,
            &mut self.batch,
            &mut self.obs,
            &mut self.statics,
            &mut self.masks,
            |_, env, slots| env.reset(source, slots, observe, blindfold),
        );
    }

    /// [`Envs::step_with`] without looking at the finished episodes.
    ///
    /// # Errors
    /// As [`Envs::step_with`].
    pub fn step(&mut self, actions: &[u16]) -> Result<StepStats, StepError> {
        self.step_with(actions, |_| {})
    }

    /// Applies `actions[i]` (an index of the action space) to env `i`, for every env still
    /// playing; reports each episode that ended to `on_end` in env order and resets its env.
    ///
    /// # Errors
    /// If the batch has the wrong length or a live env's action is not in its mask; nothing
    /// is applied then.
    pub fn step_with(
        &mut self,
        actions: &[u16],
        mut on_end: impl FnMut(&EpisodeEnd<'_>),
    ) -> Result<StepStats, StepError> {
        if actions.len() != self.batch.len() {
            return Err(StepError::Length {
                expected: self.batch.len(),
                got: actions.len(),
            });
        }
        for (env, (e, &action)) in self.batch.iter().zip(actions).enumerate() {
            let acts = e.episode.is_some() && !e.ended && live(&e.combat);
            if acts && !e.mask.contains(usize::from(action)) {
                return Err(StepError::Illegal { env, action });
            }
        }
        let observe = self.observe;
        let counts = each_env(
            self.threads,
            &mut self.batch,
            &mut self.obs,
            &mut self.statics,
            &mut self.masks,
            |i, env, slots| env.step(actions[i], slots, observe),
        );
        let mut stats = StepStats {
            actions: counts.actions,
            unmapped: counts.unmapped,
            ..StepStats::default()
        };
        for (i, env) in self.batch.iter_mut().enumerate() {
            self.done[i] = 0;
            self.won[i] = 0;
            self.final_hp[i] = 0;
            if !env.ended {
                continue;
            }
            env.ended = false;
            let Some(episode) = env.episode else {
                continue;
            };
            let end = EpisodeEnd {
                env: i,
                episode,
                seed: self.source.starts()[episode.start].seed,
                status: env.combat.status(),
                actions: env.actions,
                combat: &env.combat,
            };
            on_end(&end);
            stats.ended += 1;
            stats.won += u32::from(end.won());
            stats.capped += u32::from(env.actions >= MAX_EPISODE_ACTIONS);
            self.done[i] = 1;
            self.won[i] = u8::from(end.won());
            let s = env.combat.state();
            self.final_hp[i] =
                i16::try_from(s.creature(s.player.creature).hp.max(0)).unwrap_or(i16::MAX);
            env.episode = self.source.episode(self.next, self.seed);
            self.next += 1;
            env.fresh = true;
            stats.reset += u32::from(env.episode.is_some());
        }
        if stats.ended > 0 {
            let (source, blindfold) = (&self.source, self.blindfold);
            stats.unmapped += each_env(
                self.threads,
                &mut self.batch,
                &mut self.obs,
                &mut self.statics,
                &mut self.masks,
                |_, env, slots| {
                    if env.fresh {
                        env.reset(source, slots, observe, blindfold)
                    } else {
                        Counts::default()
                    }
                },
            )
            .unmapped;
        }
        Ok(stats)
    }

    /// The observations: env `i`'s is `obs()[i * OBS_BYTES..][..OBS_BYTES]`.
    #[must_use]
    pub fn obs(&self) -> &[u8] {
        &self.obs
    }

    /// The static observations, `STATIC_BYTES` per env, written with the observations.
    #[must_use]
    pub fn static_obs(&self) -> &[u8] {
        &self.statics
    }

    /// The masks: env `i`'s is `masks()[i * MASK_WORDS..][..MASK_WORDS]` (all zero when the
    /// env has no episode).
    #[must_use]
    pub fn masks(&self) -> &[u64] {
        &self.masks
    }

    /// Env `i`'s mask.
    #[must_use]
    pub fn mask(&self, i: usize) -> &[u64] {
        &self.masks[i * MASK_WORDS..][..MASK_WORDS]
    }

    /// Per env, 1 when its episode ended in the last step (the env was reset since).
    #[must_use]
    pub fn done(&self) -> &[u8] {
        &self.done
    }

    /// Per env, 1 when its episode ended in the last step with a win.
    #[must_use]
    pub fn won(&self) -> &[u8] {
        &self.won
    }

    /// Per env, the player's HP (0 or more) when its episode ended in the last step; 0 for the
    /// others.
    #[must_use]
    pub fn final_hp(&self) -> &[i16] {
        &self.final_hp
    }

    /// Env `i`'s episode; `None` once the source has no more for it.
    #[must_use]
    pub fn episode(&self, i: usize) -> Option<Episode> {
        self.batch[i].episode
    }

    /// The number of envs still playing an episode.
    #[must_use]
    pub fn live(&self) -> usize {
        self.batch.iter().filter(|e| e.episode.is_some()).count()
    }

    /// The source.
    #[must_use]
    pub const fn source(&self) -> &StartSource {
        &self.source
    }

    /// A copy of env `i`'s fight (for search, or to reproduce a trajectory).
    #[must_use]
    pub fn snapshot(&self, i: usize) -> Combat {
        self.batch[i].combat.clone()
    }

    /// Env `i`'s fight, read-only.
    #[must_use]
    pub fn combat(&self, i: usize) -> &Combat {
        &self.batch[i].combat
    }

    /// Puts `combat` into env `i` (its episode and action count stay) and rewrites its mask
    /// and observation. A fight that is over ends the episode at the next step.
    pub fn restore(&mut self, i: usize, combat: &Combat) {
        let env = &mut self.batch[i];
        env.combat.clone_from(combat);
        env.observer.reset();
        env.ended = false;
        let mut slots = Slots {
            obs: &mut self.obs[i * OBS_BYTES..][..OBS_BYTES],
            statics: &mut self.statics[i * STATIC_BYTES..][..STATIC_BYTES],
            mask: &mut self.masks[i * MASK_WORDS..][..MASK_WORDS],
        };
        if self.observe {
            observe_static(&env.combat, slots.statics);
        }
        if live(&env.combat) {
            env.extract(&mut slots, self.observe);
        } else {
            slots.mask.fill(0);
        }
    }
}
