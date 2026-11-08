//! The run-level environment (`docs/specs/rl-2x-plan.md`, stage 9): whole runs as episodes.
//!
//! [`RunEnvs`] owns `n` runs and flat, stable buffers like [`crate::Envs`]: the observations
//! (`n` × [`RUN_OBS_BYTES`], [`crate::run_obs`]), the masks (`n` × [`RUN_MASK_WORDS`] words over
//! the run space of [`crate::run_action`]) and, per env, `done`, `won`, how the last episode
//! ended ([`RunEnd`]) and its final floor. An episode is a run from `Run::new` on a seed of the
//! [`SeedSource`] to death, victory, unported content, a position no action of which the engine
//! accepts, or [`MAX_RUN_ACTIONS`].
//!
//! [`RunEnvs::step`] applies one index per env, then plays on by itself until the next
//! decision with more than one option in the space:
//! - a decision with exactly one option is taken automatically and counted
//!   ([`RunStepStats::auto`]);
//! - in a hybrid mode ([`CombatMode::Greedy`], [`CombatMode::Random`]) every fight is played by
//!   the env's [`Resolver`] (`resolved`), so the agent sees only run-level choices; in
//!   [`CombatMode::Agent`] it plays the fights through the combat space;
//! - a listed action the engine would refuse (`Run::apply` rejects some legal actions whose
//!   content is not ported; [`Run::accepts`] tells which) has no bit in the mask (`refused`);
//!   with no bit left the episode ends as [`RunEnd::Stuck`].
//!
//! Episodes are handed out in env order after the envs have stepped, so which env plays which
//! seed, and every trajectory, is the same for any [`RunEnvs::threads`].

use std::thread;

use sts2_core::Character;
use sts2_core::run::{Act1, Run, RunAction, RunDecision, UnlockState};

use crate::resolve::{CombatMode, Resolver};
use crate::run_action::{RUN_MASK_WORDS, RunMask, check_run_mask};
use crate::run_obs::{RUN_OBS_BYTES, RunObserver};
use crate::seeds::SeedSource;

/// Engine actions a run may take (the agent's, the automatic ones and the resolver's) before it
/// ends as [`RunEnd::Capped`].
pub const MAX_RUN_ACTIONS: u64 = 60_000;

/// How an episode ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RunEnd {
    /// Not ended.
    #[default]
    None = 0,
    /// The player died.
    Died = 1,
    /// The run was won.
    Victory = 2,
    /// The run met content the engine does not port (a fight stopped, or `Run::new` failed).
    Unsupported = 3,
    /// No option of the position is accepted.
    Stuck = 4,
    /// [`MAX_RUN_ACTIONS`] were taken.
    Capped = 5,
}

/// What the envs play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunConfig {
    /// The character, or `None` for seed `s` playing `Character::ALL[s % 5]`.
    pub character: Option<Character>,
    /// 0 to 10.
    pub ascension: u8,
    pub combat: CombatMode,
    /// Whether the combat section's intents are written.
    pub intents: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            character: Some(Character::Necrobinder),
            ascension: 0,
            combat: CombatMode::Agent,
            intents: true,
        }
    }
}

impl RunConfig {
    /// The character of seed `seed`.
    #[must_use]
    pub fn character_of(&self, seed: u64) -> Character {
        self.character
            .unwrap_or_else(|| Character::ALL[usize::try_from(seed % 5).unwrap_or_default()])
    }

    /// A new run on `seed`.
    ///
    /// # Errors
    /// As `Run::new`.
    pub fn run(&self, seed: u64) -> Result<Run, sts2_core::run::RunError> {
        Run::new(
            self.character_of(seed),
            &format!("RUN{seed}"),
            self.ascension,
            UnlockState::all(),
            Act1::Random,
        )
    }
}

/// A finished episode, as [`RunEnvs::step_with`] reports it before the env resets.
#[derive(Clone, Copy, Debug)]
pub struct RunEpisodeEnd<'a> {
    pub env: usize,
    /// The episode's serial (the `k` of [`SeedSource::seed`]) and seed.
    pub episode: u64,
    pub seed: u64,
    pub end: RunEnd,
    /// The total floor reached.
    pub floor: u16,
    /// Engine actions; of those, the agent's, the automatic ones and the resolver's.
    pub actions: u64,
    pub decisions: u64,
    pub auto: u64,
    pub resolved: u64,
    /// The final run (`None` if `Run::new` failed).
    pub run: Option<&'a Run>,
}

/// What one [`RunEnvs::step`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunStepStats {
    /// Engine actions applied: the agent's, the automatic ones and the resolver's.
    pub actions: u64,
    /// The agent's actions applied.
    pub decisions: u64,
    /// Single-option decisions taken automatically.
    pub auto: u64,
    /// Actions the combat resolver took.
    pub resolved: u64,
    /// Listed actions left out of the masks because the engine would refuse them, and resolver
    /// positions where it refused every action.
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

/// Why [`RunEnvs::step`] refused a batch; nothing was applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStepError {
    /// The batch does not have one action per env.
    Length { expected: usize, got: usize },
    /// The action of a live env is not set in its mask.
    Illegal { env: usize, action: u16 },
}

impl std::fmt::Display for RunStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Length { expected, got } => write!(f, "expected {expected} actions, got {got}"),
            Self::Illegal { env, action } => write!(f, "action {action} is not legal in env {env}"),
        }
    }
}

impl std::error::Error for RunStepError {}

/// What the envs of one pass did, summed.
#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    actions: u64,
    decisions: u64,
    auto: u64,
    resolved: u64,
    refused: u64,
    unmapped: u64,
}

impl std::ops::Add for Counts {
    type Output = Self;

    fn add(self, o: Self) -> Self {
        Self {
            actions: self.actions + o.actions,
            decisions: self.decisions + o.decisions,
            auto: self.auto + o.auto,
            resolved: self.resolved + o.resolved,
            refused: self.refused + o.refused,
            unmapped: self.unmapped + o.unmapped,
        }
    }
}

/// One env's slices of the batch buffers.
struct Slots<'a> {
    obs: &'a mut [u8],
    mask: &'a mut [u64],
}

/// One environment.
#[derive(Debug)]
struct RunEnv {
    /// The run of the episode (`None` before the first, or if `Run::new` failed).
    run: Option<Run>,
    observer: RunObserver,
    resolver: Option<Box<dyn Resolver>>,
    legal: Vec<RunAction>,
    mask: RunMask,
    /// The episode's serial and seed; `None` once the source has no more.
    episode: Option<(u64, u64)>,
    actions: u64,
    decisions: u64,
    auto: u64,
    resolved: u64,
    end: RunEnd,
    /// A new episode was assigned in this step.
    fresh: bool,
}

impl RunEnv {
    fn new(config: RunConfig) -> Self {
        Self {
            run: None,
            observer: RunObserver::new(config.intents),
            resolver: config.combat.resolver(),
            legal: Vec::new(),
            mask: RunMask::new(),
            episode: None,
            actions: 0,
            decisions: 0,
            auto: 0,
            resolved: 0,
            end: RunEnd::None,
            fresh: false,
        }
    }

    /// Whether the env takes an action this step.
    const fn acts(&self) -> bool {
        self.episode.is_some() && matches!(self.end, RunEnd::None) && self.run.is_some()
    }

    /// Applies the legal action at list position `position`, one the mask holds.
    fn apply(&mut self, position: usize, counts: &mut Counts) {
        let run = self.run.as_mut().expect("a live run");
        run.apply(self.legal[position].clone())
            .expect("the mask holds only actions the engine accepts");
        self.actions += 1;
        counts.actions += 1;
    }

    /// How the run ended, if it has.
    fn terminal(&self) -> Option<RunEnd> {
        let run = self.run.as_ref()?;
        match run.pending_decision() {
            RunDecision::GameOver => Some(RunEnd::Died),
            RunDecision::Victory => Some(RunEnd::Victory),
            RunDecision::Stopped(_) => Some(RunEnd::Unsupported),
            _ if self.actions >= MAX_RUN_ACTIONS => Some(RunEnd::Capped),
            _ => None,
        }
    }

    /// The resolver's next action in the fight (it may answer a card choice with an action the
    /// list does not hold), or the first legal one the engine accepts.
    fn resolve(&mut self, counts: &mut Counts) -> bool {
        let run = self.run.as_mut().expect("a live run");
        let resolver = self.resolver.as_mut().expect("a resolver");
        let action = resolver.act(run, &self.legal);
        let applied = run.apply(action).is_ok()
            || self.legal.iter().find(|a| run.accepts(a)).is_some_and(|a| {
                run.apply(a.clone()).expect("an accepted action");
                true
            });
        if applied {
            self.actions += 1;
            self.resolved += 1;
            counts.actions += 1;
            counts.resolved += 1;
        } else {
            counts.refused += 1;
        }
        applied
    }

    /// Plays on from the current position (resolving fights, taking single options) until a
    /// decision with more than one option or the episode's end; then writes the mask and, if
    /// the episode goes on, the observation.
    fn settle(&mut self, slots: &mut Slots<'_>, counts: &mut Counts) {
        loop {
            if let Some(end) = self.terminal() {
                self.end = end;
                return;
            }
            let run = self.run.as_ref().expect("a live run");
            run.legal_actions_into(&mut self.legal);
            if self.resolver.is_some() && matches!(run.pending_decision(), RunDecision::Combat(_)) {
                if self.legal.is_empty() || !self.resolve(counts) {
                    self.end = RunEnd::Stuck;
                    return;
                }
                continue;
            }
            counts.unmapped += u64::from(self.mask.fill_run(run, &self.legal));
            counts.refused += u64::from(self.mask.retain_accepted(run, &self.legal));
            debug_assert_eq!(
                check_run_mask(run, &self.legal, &self.mask),
                Ok(()),
                "the run mask disagrees with the legal list"
            );
            match self.mask.count() {
                0 => {
                    self.end = RunEnd::Stuck;
                    return;
                }
                1 => {
                    let index = self.mask.nth(0).expect("one bit");
                    let position = self.mask.position(index).expect("set");
                    self.apply(position, counts);
                    self.auto += 1;
                    counts.auto += 1;
                }
                _ => break,
            }
        }
        slots.mask.copy_from_slice(self.mask.bits());
        let run = self.run.as_ref().expect("a live run");
        self.observer.observe(run, slots.obs);
    }

    /// Applies the agent's `action` (checked against the mask), then settles.
    fn step(&mut self, action: u16, slots: &mut Slots<'_>) -> Counts {
        let mut counts = Counts::default();
        if !self.acts() {
            return counts;
        }
        let index = usize::from(action);
        let position = self
            .mask
            .position(index)
            .expect("the batch was checked against the masks");
        self.apply(position, &mut counts);
        self.decisions += 1;
        counts.decisions = 1;
        self.settle(slots, &mut counts);
        counts
    }

    /// Starts the fresh episode: a new run on its seed, settled to its first decision.
    fn reset(&mut self, config: RunConfig, slots: &mut Slots<'_>) -> Counts {
        self.fresh = false;
        self.end = RunEnd::None;
        (self.actions, self.decisions, self.auto, self.resolved) = (0, 0, 0, 0);
        self.observer.reset();
        slots.mask.fill(0);
        slots.obs.fill(0);
        let Some((_, seed)) = self.episode else {
            self.run = None;
            return Counts::default();
        };
        if let Some(resolver) = &mut self.resolver {
            resolver.reset(seed);
        }
        let mut counts = Counts::default();
        self.run = config.run(seed).ok();
        if self.run.is_some() {
            self.settle(slots, &mut counts);
        } else {
            self.end = RunEnd::Unsupported;
        }
        counts
    }

    /// The total floor reached.
    fn floor(&self) -> u16 {
        self.run.as_ref().map_or(0, |run| {
            u16::try_from(run.state().total_floor()).unwrap_or(u16::MAX)
        })
    }
}

/// Runs `f` on every env with its index and slots, on up to `threads` scoped threads (each
/// owning a contiguous share of the envs); the sum of its results.
fn each_env<F>(
    threads: usize,
    envs: &mut [RunEnv],
    obs: &mut [u8],
    masks: &mut [u64],
    f: F,
) -> Counts
where
    F: Fn(usize, &mut RunEnv, &mut Slots<'_>) -> Counts + Sync,
{
    let run = |first: usize, envs: &mut [RunEnv], obs: &mut [u8], masks: &mut [u64]| {
        envs.iter_mut()
            .zip(obs.as_chunks_mut::<RUN_OBS_BYTES>().0)
            .zip(masks.as_chunks_mut::<RUN_MASK_WORDS>().0)
            .enumerate()
            .map(|(i, ((env, obs), mask))| f(first + i, env, &mut Slots { obs, mask }))
            .fold(Counts::default(), |sum, c| sum + c)
    };
    if threads <= 1 || envs.len() < 2 {
        return run(0, envs, obs, masks);
    }
    let per = envs.len().div_ceil(threads);
    thread::scope(|scope| {
        let handles: Vec<_> = envs
            .chunks_mut(per)
            .zip(obs.chunks_mut(per * RUN_OBS_BYTES))
            .zip(masks.chunks_mut(per * RUN_MASK_WORDS))
            .enumerate()
            .map(|(t, ((envs, obs), masks))| {
                let run = &run;
                scope.spawn(move || run(t * per, envs, obs, masks))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("an env thread panicked"))
            .fold(Counts::default(), |sum, c| sum + c)
    })
}

/// A batch of run environments.
#[derive(Debug)]
pub struct RunEnvs {
    batch: Vec<RunEnv>,
    seeds: SeedSource,
    config: RunConfig,
    /// The next episode's serial.
    next: u64,
    threads: usize,
    obs: Vec<u8>,
    /// The observation of the episode that ended in the last step, before its reset.
    final_obs: Vec<u8>,
    masks: Vec<u64>,
    done: Vec<u8>,
    won: Vec<u8>,
    ends: Vec<u8>,
    floors: Vec<u16>,
}

impl RunEnvs {
    /// `n` envs playing the runs of `seeds` (in order) as `config` says, reset to their first
    /// episodes.
    #[must_use]
    pub fn new(n: usize, seeds: SeedSource, config: RunConfig) -> Self {
        let mut envs = Self {
            batch: (0..n).map(|_| RunEnv::new(config)).collect(),
            seeds,
            config,
            next: 0,
            threads: 1,
            obs: vec![0; n * RUN_OBS_BYTES],
            final_obs: vec![0; n * RUN_OBS_BYTES],
            masks: vec![0; n * RUN_MASK_WORDS],
            done: vec![0; n],
            won: vec![0; n],
            ends: vec![0; n],
            floors: vec![0; n],
        };
        envs.reset_all();
        envs
    }

    /// Replaces env `i`'s combat resolver (a hybrid mode's own policy); it takes effect at the
    /// next fight. `None` lets the agent play combat.
    pub fn set_resolver(&mut self, i: usize, resolver: Option<Box<dyn Resolver>>) {
        self.batch[i].resolver = resolver;
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

    /// Steps the envs on up to `t` threads (at least 1).
    pub fn threads(&mut self, t: usize) {
        self.threads = t.max(1);
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &RunConfig {
        &self.config
    }

    /// Hands env `i` the next episode of the source.
    fn assign(&mut self, i: usize) {
        let k = self.next;
        self.next += 1;
        let env = &mut self.batch[i];
        env.episode = self.seeds.seed(k).map(|seed| (k, seed));
        env.fresh = true;
    }

    /// Resets the envs marked fresh.
    fn reset_fresh(&mut self) -> Counts {
        let config = self.config;
        each_env(
            self.threads,
            &mut self.batch,
            &mut self.obs,
            &mut self.masks,
            |_, env, slots| {
                if env.fresh {
                    env.reset(config, slots)
                } else {
                    Counts::default()
                }
            },
        )
    }

    /// Starts a new episode in every env (the next seeds of the source, in env order).
    pub fn reset_all(&mut self) {
        for i in 0..self.batch.len() {
            self.assign(i);
        }
        for buffer in [&mut self.done, &mut self.won, &mut self.ends] {
            buffer.fill(0);
        }
        self.floors.fill(0);
        self.final_obs.fill(0);
        self.reset_fresh();
    }

    /// [`RunEnvs::step_with`] without looking at the finished episodes.
    ///
    /// # Errors
    /// As [`RunEnvs::step_with`].
    pub fn step(&mut self, actions: &[u16]) -> Result<RunStepStats, RunStepError> {
        self.step_with(actions, |_| {})
    }

    /// Applies `actions[i]` (an index of the run space) to env `i` for every env still playing,
    /// plays on to each env's next decision, reports each episode that ended to `on_end` in env
    /// order and resets its env.
    ///
    /// # Errors
    /// If the batch has the wrong length or a live env's action is not in its mask; nothing
    /// is applied then.
    pub fn step_with(
        &mut self,
        actions: &[u16],
        mut on_end: impl FnMut(&RunEpisodeEnd<'_>),
    ) -> Result<RunStepStats, RunStepError> {
        if actions.len() != self.batch.len() {
            return Err(RunStepError::Length {
                expected: self.batch.len(),
                got: actions.len(),
            });
        }
        for (env, (e, &action)) in self.batch.iter().zip(actions).enumerate() {
            if e.acts() && !e.mask.contains(usize::from(action)) {
                return Err(RunStepError::Illegal { env, action });
            }
        }
        let counts = each_env(
            self.threads,
            &mut self.batch,
            &mut self.obs,
            &mut self.masks,
            |i, env, slots| env.step(actions[i], slots),
        );
        let mut stats = RunStepStats::default();
        let mut counts = counts;
        // Ended episodes, then their envs' next ones; an episode that ends at its reset
        // (`Run::new` failed, or no decision before the end) ends in the next step.
        for i in 0..self.batch.len() {
            self.done[i] = 0;
            self.won[i] = 0;
            self.ends[i] = 0;
            self.floors[i] = 0;
            let env = &mut self.batch[i];
            if env.end == RunEnd::None {
                continue;
            }
            // Keep the ended episode's observation before `assign`/`reset_fresh` overwrites it.
            self.final_obs[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES]
                .copy_from_slice(&self.obs[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES]);
            let Some((episode, seed)) = env.episode else {
                continue;
            };
            let end = RunEpisodeEnd {
                env: i,
                episode,
                seed,
                end: env.end,
                floor: env.floor(),
                actions: env.actions,
                decisions: env.decisions,
                auto: env.auto,
                resolved: env.resolved,
                run: env.run.as_ref(),
            };
            on_end(&end);
            stats.ended += 1;
            stats.won += u32::from(end.end == RunEnd::Victory);
            stats.capped += u32::from(end.end == RunEnd::Capped);
            stats.stuck += u32::from(end.end == RunEnd::Stuck);
            self.done[i] = 1;
            self.won[i] = u8::from(end.end == RunEnd::Victory);
            self.ends[i] = end.end as u8;
            self.floors[i] = end.floor;
            env.end = RunEnd::None;
            self.assign(i);
            stats.reset += u32::from(self.batch[i].episode.is_some());
        }
        if stats.ended > 0 {
            counts = counts + self.reset_fresh();
        }
        stats.actions = counts.actions;
        stats.decisions = counts.decisions;
        stats.auto = counts.auto;
        stats.resolved = counts.resolved;
        stats.refused = counts.refused;
        stats.unmapped = counts.unmapped;
        Ok(stats)
    }

    /// The observations: env `i`'s is `obs()[i * RUN_OBS_BYTES..][..RUN_OBS_BYTES]`.
    #[must_use]
    pub fn obs(&self) -> &[u8] {
        &self.obs
    }

    /// The observation of the episode that ended in the last step, per env (the current
    /// observation after the reset is in [`RunEnvs::obs`]). Zeros until an episode ends.
    #[must_use]
    pub fn final_obs(&self) -> &[u8] {
        &self.final_obs
    }

    /// The masks: env `i`'s is `masks()[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]` (all zero when
    /// the env has no episode).
    #[must_use]
    pub fn masks(&self) -> &[u64] {
        &self.masks
    }

    /// Env `i`'s mask.
    #[must_use]
    pub fn mask(&self, i: usize) -> &[u64] {
        &self.masks[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]
    }

    /// Per env, 1 when its episode ended in the last step (the env was reset since).
    #[must_use]
    pub fn done(&self) -> &[u8] {
        &self.done
    }

    /// Per env, 1 when its episode ended in the last step with a victory.
    #[must_use]
    pub fn won(&self) -> &[u8] {
        &self.won
    }

    /// Per env, how the episode that ended in the last step ended ([`RunEnd`] as `u8`, 0 if
    /// none did).
    #[must_use]
    pub fn ends(&self) -> &[u8] {
        &self.ends
    }

    /// Per env, the total floor the episode that ended in the last step reached.
    #[must_use]
    pub fn floors(&self) -> &[u16] {
        &self.floors
    }

    /// Env `i`'s episode serial and seed; `None` once the source has no more for it.
    #[must_use]
    pub fn episode(&self, i: usize) -> Option<(u64, u64)> {
        self.batch[i].episode
    }

    /// The number of envs still playing an episode.
    #[must_use]
    pub fn live(&self) -> usize {
        self.batch.iter().filter(|e| e.episode.is_some()).count()
    }

    /// Env `i`'s run, read-only.
    #[must_use]
    pub fn run(&self, i: usize) -> Option<&Run> {
        self.batch[i].run.as_ref()
    }

    /// Env `i`'s current legal list, for checking its mask.
    #[must_use]
    pub fn legal(&self, i: usize) -> &[RunAction] {
        &self.batch[i].legal
    }
}
