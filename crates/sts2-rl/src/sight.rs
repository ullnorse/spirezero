//! One run seen as a [`RunEnvs`](crate::RunEnvs) env sees it, outside the env (WEB82: the
//! combat net playing in the web client).
//!
//! [`RunSight::settle`] builds the mask of a position the way the env's `settle` does (the legal
//! list, the discard offers, the refused indices cleared, the classes merged), and
//! [`RunSight::see`] writes its observation and plan column. The observer keeps per-fight
//! history (the full view's past intents, [`crate::obs_full`]), which the env records at every
//! decision it shows the agent: a caller stepping a run itself calls [`RunSight::step`] at every
//! position, which observes exactly the positions the env would (more than one class).

use sts2_core::combat::Action;
use sts2_core::run::{Run, RunAction};

use crate::RUN_ACTIONS;
use crate::classes::DedupeScratch;
use crate::run_action::{RunMask, discard_offers};
use crate::run_envs::{RunOptions, dedupe, write_plan_section};
use crate::run_obs::{RUN_OBS_BYTES, RunObserver};

/// A run's env view: its observer, and the last settled position's legal list and mask.
#[derive(Clone, Debug)]
pub struct RunSight {
    options: RunOptions,
    observer: RunObserver,
    legal: Vec<RunAction>,
    mask: RunMask,
    counts: Vec<u8>,
    scratch: DedupeScratch,
    plan: Vec<(usize, Action)>,
    obs: Vec<u8>,
}

impl RunSight {
    /// A sight with the env options `options` (the intents written, as the FFI's envs).
    #[must_use]
    pub fn new(options: RunOptions) -> Self {
        Self {
            options,
            observer: RunObserver::new(true)
                .with_v2(options.obs_v2)
                .with_full(options.obs_full)
                .with_events(options.obs_events),
            legal: Vec::new(),
            mask: RunMask::new(),
            counts: vec![0; RUN_ACTIONS],
            scratch: DedupeScratch::default(),
            plan: Vec::new(),
            obs: vec![0; RUN_OBS_BYTES],
        }
    }

    /// The options of the CB3 combat net's envs: [`RunOptions::E2E`] with the v2 section, its
    /// plan column (one play), the full view and the discard offers.
    #[must_use]
    pub const fn cb3() -> RunOptions {
        let mut o = RunOptions::E2E;
        o.obs_v2 = true;
        o.plan_column = true;
        o.obs_full = true;
        o.discard_potion = true;
        o.plan_depth = 1;
        o
    }

    /// Starts a new episode (forgets the fight's history).
    pub fn reset(&mut self) {
        self.observer.reset();
    }

    /// Builds the mask of `run`'s position as the env does; the number of classes left.
    pub fn settle(&mut self, run: &Run) -> u32 {
        run.legal_actions_into(&mut self.legal);
        if self.options.discard_potion {
            discard_offers(run, &mut self.legal);
        }
        self.mask.fill_run(run, &self.legal);
        self.mask.retain_accepted(run, &self.legal);
        if self.options.dedupe {
            dedupe(
                run,
                &self.legal,
                &mut self.mask,
                &mut self.counts,
                &mut self.scratch,
                self.options.pile_key,
            );
        }
        self.mask.count()
    }

    /// Writes the observation of `run`'s position (settled by [`RunSight::settle`]) and its
    /// plan column, as the env does when it shows the position to the agent.
    pub fn see(&mut self, run: &Run) -> &[u8] {
        self.observer.observe(run, &mut self.obs);
        write_plan_section(
            run,
            self.options,
            &self.mask,
            &self.legal,
            &mut self.plan,
            &mut self.obs,
        );
        &self.obs
    }

    /// One position of a run the caller plays: settles it and, where the env would show it to
    /// the agent (more than one class), observes it. Whether it observed.
    pub fn step(&mut self, run: &Run) -> bool {
        let shown = self.settle(run) > 1;
        if shown {
            self.see(run);
        }
        shown
    }

    /// The last observation ([`RUN_OBS_BYTES`]).
    #[must_use]
    pub fn obs(&self) -> &[u8] {
        &self.obs
    }

    /// The settled mask.
    #[must_use]
    pub const fn mask(&self) -> &RunMask {
        &self.mask
    }

    /// The action of a set mask index (a class's first member).
    #[must_use]
    pub fn action(&self, index: usize) -> Option<&RunAction> {
        self.legal.get(self.mask.position(index)?)
    }
}
