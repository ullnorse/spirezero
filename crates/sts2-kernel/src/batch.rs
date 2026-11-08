//! Lockstep stepping of many fights (perf-plan item 11).
//!
//! [`run_lockstep`] advances `N` independent fights one action at a time each, round-robin, so
//! that the fights' dependency chains overlap in the CPU instead of running one after another.
//! The fights' control flow diverges at almost every step (different hands, monsters, piles),
//! so there is no data parallelism to vectorise: a structure-of-arrays layout would scatter
//! each step's reads over `N` arrays without letting any two fights share an instruction. The
//! batch keeps each fight contiguous (one `Copy` struct per lane) and only interleaves them.

use crate::Actions;
use crate::state::{Action, Fight, Status};
use crate::tables::Kernel;

/// One lane of a lockstep batch: a fight and the policy that picks its actions.
pub trait Lane {
    /// The lane's fight.
    fn fight(&mut self) -> &mut Fight;
    /// Picks one of `legal` (never empty).
    fn pick(&mut self, legal: &[Action]) -> Action;
    /// Called when the fight ended (`Won`, `Lost` or `Unsupported`) or reached `max_actions`;
    /// returns whether the lane starts another fight (it then resets [`Lane::fight`]).
    fn finished(&mut self, actions: u64) -> bool;
}

/// Plays every lane's fights to the end, one action per lane per round, until no lane starts
/// another fight. A fight stops after `max_actions`.
pub fn run_lockstep<L: Lane>(k: &Kernel, lanes: &mut [L], max_actions: u64) {
    let mut live: Vec<(usize, u64)> = (0..lanes.len()).map(|i| (i, 0)).collect();
    let mut legal = Actions::new();
    while !live.is_empty() {
        let mut i = 0;
        while i < live.len() {
            let (lane, actions) = &mut live[i];
            let l = &mut lanes[*lane];
            let f = l.fight();
            let playing = matches!(f.status, Status::PlayerTurn | Status::Decision);
            if playing && *actions < max_actions {
                f.legal_actions_into(k, &mut legal);
                let action = l.pick(legal.as_slice());
                l.fight().step_unchecked(k, action);
                *actions += 1;
                i += 1;
            } else if l.finished(*actions) {
                *actions = 0;
                i += 1;
            } else {
                live.swap_remove(i);
            }
        }
    }
}
