//! The RL interface over sts2-core's combat (`docs/specs/rl-2x-plan.md`, section 7): the fixed
//! action space and its mask ([`action`]), the observation ([`obs`]) with its cache of the
//! enemies' intents ([`intents`]), the [`Envs`] batch
//! ([`envs`]) and the episode starts ([`start`]). Content kinds in the observation are the
//! append-only ids of [`vocab`]; [`invariants`] has the checks the FFI fuzz target runs.
//!
//! The run-level environment (stage 9) plays whole runs: [`RunEnvs`] ([`run_envs`]) over the
//! run space ([`run_action`], appended after the combat one), the run observation
//! ([`run_obs`]), seeds ([`seeds`]) and the combat resolvers of its hybrid mode ([`resolve`]).
//!
//! For the end-to-end learner, [`RunOptions`] adds action classes ([`classes`]), caps, reward
//! events and counters to the run env, and [`entity`] maps each run-space index to the entity it
//! acts on, and [`practice`] restarts reseeded snapshots taken before elite and boss fights.
//!
//! Nothing here changes a fight: the mask is derived from `legal_actions`, the observation
//! only reads the state, and the intent cache is checked against the slow path on every
//! observation in debug builds (and the `regress` profile).

pub mod action;
pub mod classes;
pub mod content_stats;
pub mod entity;
pub mod envs;
pub mod human;
pub mod intents;
pub mod invariants;
mod key;
mod listeners;
pub mod obs;
pub mod obs_events;
pub mod obs_full;
pub mod obs_v2;
pub mod plan_column;
pub mod practice;
pub mod priv_obs;
pub mod resolve;
pub mod run_action;
pub mod run_envs;
pub mod run_obs;
pub mod seeds;
pub mod sight;
pub mod start;
mod step_timing;
pub mod vocab;

pub use action::{ACTIONS, IndexMask, MASK_WORDS, Mask};
pub use envs::{Envs, EpisodeEnd, MAX_EPISODE_ACTIONS, StepError, StepStats, check_mask};
pub use human::{GYM_CELL_NAMES, GYM_CELLS, Gym, GymCell, GymConfig, GymStats, HumanStart};
pub use invariants::check_invariants;
pub use obs::{OBS_BYTES, Observer, STATIC_BYTES, observe_static};
pub use practice::{Cell, PracticeConfig, PracticeStats};
pub use resolve::{CombatMode, Resolver};
pub use run_action::{RUN_ACTIONS, RUN_MASK_WORDS, RunMask, check_run_mask};
pub use run_envs::{
    HOLD, MAX_RUN_ACTIONS, PRACTICE_EPISODE, Requests, RunConfig, RunEnd, RunEnvs, RunEpisodeEnd,
    RunEvents, RunOptions, RunStepError, RunStepStats, SATURATIONS, Search, SearchConfig,
    SearchResult, SearchStats, World,
};
pub use run_obs::{RUN_OBS_BYTES, RunObserver};
pub use seeds::SeedSource;
pub use start::{Episode, Order, Start, StartRecord, StartSource};
