//! Practice starts (`docs/plans/necro-e2e.md`, row "Practice", decision D6): reseeded snapshots
//! of the learner's own runs, taken just before an elite or boss fight.
//!
//! The method is spire-mentor's Instinct (`sts2-pycli/python/ppo/e2e/practice.py` there):
//! - **Capture** ([`capture`]): when a normal run takes the map move into an elite or boss point,
//!   the run as it was before the move, plus the move, is a [`Snapshot`] of one of six [`Cell`]s
//!   (elite or boss, acts 1-3). The second boss of a double-boss act has no cell: a start before
//!   the first boss plays it.
//! - **Start** ([`start`]): the snapshot's run with fresh run and player streams from the reseed
//!   (`PRACTICE<reseed>`), then the move. The rooms are rolled at run start, so the encounter is
//!   the same; the shuffles, the monsters' AI (their streams hash the run seed) and the rewards
//!   differ.
//! - **End** ([`SegmentEnd`]): a start ends at its first decision outside combat after the fight
//!   ([`crate::RunEnd::PracticeEnd`], a truncation the learner bootstraps from that position's
//!   value), or at death; a start before the last act's boss plays to the run's end.
//! - **Pool** ([`Pool`]): per cell a first-in, first-out queue of [`PracticeConfig::pool`]
//!   snapshots (the most recent kept); an entry leaves after [`PracticeConfig::max_uses`] starts.
//!   A start draws a cell by [`PracticeConfig::weights`]; a cell with fewer than
//!   [`PracticeConfig::min_active`] entries falls back to the same kind's lower acts, then to a
//!   normal run. The entry within a cell is drawn uniformly. Every draw comes from one
//!   [`SplitMix`] on [`PracticeConfig::seed`], on the caller's thread, so starts do not depend on
//!   the env threads.
//!
//! The observation of a practice start is the ordinary one: nothing in it says "practice".

use std::collections::VecDeque;

use sts2_core::rng::{PlayerRngSet, RunRngSet, canonicalize_seed};
use sts2_core::run::{MapPointType, Run, RunAction, RunDecision, RunError};

use crate::seeds::SplitMix;

/// The number of cells.
pub const CELLS: usize = 6;

/// The cells' names, by [`Cell::index`].
pub const CELL_NAMES: [&str; CELLS] = [
    "ELITE_ACT1",
    "ELITE_ACT2",
    "ELITE_ACT3",
    "BOSS_ACT1",
    "BOSS_ACT2",
    "BOSS_ACT3",
];

/// Where a snapshot was taken: before an elite or a boss of act index `act` (0 to 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub boss: bool,
    pub act: u8,
}

impl Cell {
    /// 0-2 the elites of acts 1-3, 3-5 the bosses ([`CELL_NAMES`]).
    #[must_use]
    pub const fn index(self) -> usize {
        self.boss as usize * 3 + self.act as usize
    }

    /// The cell of index `i` (below [`CELLS`]).
    #[must_use]
    pub const fn from_index(i: usize) -> Option<Self> {
        if i >= CELLS {
            return None;
        }
        Some(Self {
            boss: i >= 3,
            #[allow(clippy::cast_possible_truncation, reason = "i % 3 < 3")]
            act: (i % 3) as u8,
        })
    }

    /// The cells a start of this one falls back to, in order: the same kind's lower acts.
    fn fallbacks(self) -> impl Iterator<Item = Self> {
        (0..=self.act).rev().map(move |act| Self { act, ..self })
    }
}

/// Where a practice start ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SegmentEnd {
    /// At its first decision outside combat (a truncation), or at the run's end.
    FirstOutOfCombat,
    /// At the run's end (a start before the last act's boss).
    Terminal,
}

/// A run just before the map move into an elite or boss point.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// The run before the move (on the map).
    pub run: Run,
    /// The move ([`RunAction::ChooseNode`]).
    pub choice: RunAction,
    pub cell: Cell,
    pub end: SegmentEnd,
}

/// The snapshot of `run` if `action` is the map move into an elite or boss point of acts 1-3
/// (but not a double-boss act's second boss).
#[must_use]
pub fn capture(run: &Run, action: &RunAction) -> Option<Snapshot> {
    let RunAction::ChooseNode(coord) = action else {
        return None;
    };
    if !matches!(run.pending_decision(), RunDecision::Map(_)) {
        return None;
    }
    let s = run.state();
    let boss = match s.map.point(*coord)?.kind {
        MapPointType::Elite => false,
        MapPointType::Boss => true,
        _ => return None,
    };
    if s.map.second_boss().is_some_and(|p| p.coord == *coord) {
        return None;
    }
    let act = u8::try_from(s.act_index).ok().filter(|&a| a < 3)?;
    let end = if boss && s.act_index + 1 == s.acts.len() {
        SegmentEnd::Terminal
    } else {
        SegmentEnd::FirstOutOfCombat
    };
    Some(Snapshot {
        run: run.clone(),
        choice: action.clone(),
        cell: Cell { boss, act },
        end,
    })
}

/// The seed string of reseed `reseed`.
#[must_use]
pub fn reseed_string(reseed: u64) -> String {
    canonicalize_seed(&format!("PRACTICE{reseed}"))
}

/// `snap`'s run with its run and player streams replaced by fresh ones from `reseed`, after its
/// move: the fight of the cell, set up on the new streams.
///
/// # Errors
/// If the run is not on the map or the move fails (it did not in the captured run).
pub fn start(snap: &Snapshot, reseed: u64) -> Result<Run, RunError> {
    let mut run = snap.run.clone();
    let state = run.loadout_state()?;
    let rng = RunRngSet::new(&reseed_string(reseed));
    state.player_rng = PlayerRngSet::for_slot(rng.seed(), 0);
    state.rng = rng;
    run.apply(snap.choice.clone())?;
    Ok(run)
}

/// The pool's settings ([`crate::RunEnvs::set_practice`]); Python recomputes the schedule's
/// `slots` and `weights` each iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PracticeConfig {
    /// Envs `0..slots` start a practice segment when their episode ends (if a cell is active).
    pub slots: usize,
    /// Relative weights of the cells for one start, by [`Cell::index`] (all 0: no starts).
    pub weights: [u32; CELLS],
    /// Snapshots kept per cell (the most recent).
    pub pool: usize,
    /// Starts per snapshot before it leaves the pool (0: no limit).
    pub max_uses: u32,
    /// Entries a cell needs to be drawn from.
    pub min_active: usize,
    /// Whether normal runs capture snapshots.
    pub capture: bool,
    /// The seed of the draws and reseeds (used when the pool is made).
    pub seed: u64,
}

impl Default for PracticeConfig {
    /// spire-mentor's sizes, no slots and the schedule's start (bosses 40%: 15/20/65 by act;
    /// elites 60%: 20/40/40), in parts per million.
    fn default() -> Self {
        Self {
            slots: 0,
            weights: [120_000, 240_000, 240_000, 60_000, 80_000, 260_000],
            pool: 32,
            max_uses: 4,
            min_active: 8,
            capture: true,
            seed: 0,
        }
    }
}

/// Counts per cell, by [`Cell::index`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PracticeStats {
    /// Snapshots taken.
    pub captured: [u64; CELLS],
    /// Snapshots pushed out of a full queue.
    pub evicted: [u64; CELLS],
    /// Snapshots removed after their last use.
    pub used_up: [u64; CELLS],
    /// Draws of the cell (before fallbacks).
    pub wanted: [u64; CELLS],
    /// Starts made from the cell (after fallbacks).
    pub started: [u64; CELLS],
    /// Draws of the cell that fell back to a lower act.
    pub fell_back: [u64; CELLS],
    /// Draws of the cell that fell back to a normal run.
    pub to_run: [u64; CELLS],
    /// Starts of the cell whose move failed.
    pub bad_start: [u64; CELLS],
}

/// A practice segment ready to play.
#[derive(Debug)]
pub struct PracticeStart {
    /// The run after the move (in the fight).
    pub run: Run,
    pub cell: Cell,
    pub end: SegmentEnd,
    pub reseed: u64,
}

#[derive(Debug)]
struct Entry {
    snap: Snapshot,
    uses: u32,
}

/// The snapshots and the draws (see the module docs).
#[derive(Debug)]
pub struct Pool {
    config: PracticeConfig,
    cells: [VecDeque<Entry>; CELLS],
    rng: SplitMix,
    stats: PracticeStats,
}

impl Pool {
    /// An empty pool.
    #[must_use]
    pub fn new(config: PracticeConfig) -> Self {
        Self {
            config,
            cells: std::array::from_fn(|_| VecDeque::new()),
            rng: SplitMix::new(config.seed ^ 0x5052_4143_5449_4345),
            stats: PracticeStats::default(),
        }
    }

    /// The settings.
    #[must_use]
    pub const fn config(&self) -> &PracticeConfig {
        &self.config
    }

    /// New settings; the snapshots, the draws' generator and the counts stay.
    pub fn configure(&mut self, config: PracticeConfig) {
        self.config = config;
        for cell in &mut self.cells {
            while cell.len() > config.pool {
                cell.pop_front();
            }
        }
    }

    /// The counts so far.
    #[must_use]
    pub const fn stats(&self) -> &PracticeStats {
        &self.stats
    }

    /// The snapshots held per cell.
    #[must_use]
    pub fn sizes(&self) -> [usize; CELLS] {
        std::array::from_fn(|i| self.cells[i].len())
    }

    /// The snapshots `cell` holds, oldest first.
    pub fn snapshots(&self, cell: Cell) -> impl Iterator<Item = &Snapshot> {
        self.cells[cell.index()].iter().map(|e| &e.snap)
    }

    /// Files `snap` in its cell, pushing out the oldest of a full queue.
    pub fn add(&mut self, snap: Snapshot) {
        let i = snap.cell.index();
        if self.config.pool == 0 {
            return;
        }
        if self.cells[i].len() >= self.config.pool {
            self.cells[i].pop_front();
            self.stats.evicted[i] += 1;
        }
        self.cells[i].push_back(Entry { snap, uses: 0 });
        self.stats.captured[i] += 1;
    }

    fn active(&self, cell: Cell) -> bool {
        let n = self.cells[cell.index()].len();
        n > 0 && n >= self.config.min_active
    }

    /// A cell drawn by the weights, or `None` if they are all 0.
    fn draw(&mut self) -> Option<Cell> {
        let total: u64 = self.config.weights.iter().map(|&w| u64::from(w)).sum();
        if total == 0 {
            return None;
        }
        let mut x = self.rng.next() % total;
        for (i, &w) in self.config.weights.iter().enumerate() {
            let w = u64::from(w);
            if x < w {
                return Cell::from_index(i);
            }
            x -= w;
        }
        None
    }

    /// One use of a uniformly drawn entry of `cell` (removed after its last use).
    fn take(&mut self, cell: Cell) -> Snapshot {
        let i = cell.index();
        let k = self.rng.below(self.cells[i].len());
        let entry = &mut self.cells[i][k];
        entry.uses += 1;
        if self.config.max_uses > 0 && entry.uses >= self.config.max_uses {
            self.stats.used_up[i] += 1;
            self.cells[i].remove(k).expect("drawn index").snap
        } else {
            entry.snap.clone()
        }
    }

    /// A practice start: a drawn cell (with fallbacks) and a reseeded snapshot of it; `None`
    /// when no cell is active (the env plays a normal run).
    pub fn start(&mut self) -> Option<PracticeStart> {
        let wanted = self.draw()?;
        self.stats.wanted[wanted.index()] += 1;
        for cell in wanted.fallbacks() {
            if !self.active(cell) {
                continue;
            }
            if cell != wanted {
                self.stats.fell_back[wanted.index()] += 1;
            }
            // A bad start (its move failed) tries another entry, at most 4 times.
            for _ in 0..4 {
                if !self.active(cell) {
                    break;
                }
                let snap = self.take(cell);
                let reseed = self.rng.next() >> 24;
                if let Ok(run) = start(&snap, reseed) {
                    self.stats.started[cell.index()] += 1;
                    return Some(PracticeStart {
                        run,
                        cell,
                        end: snap.end,
                        reseed,
                    });
                }
                self.stats.bad_start[cell.index()] += 1;
            }
        }
        self.stats.to_run[wanted.index()] += 1;
        None
    }
}
