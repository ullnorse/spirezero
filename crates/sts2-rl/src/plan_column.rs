//! RLN22: a heuristic score for every legal combat action (our version of spire-mentor's
//! priority score), from the beam-search turn planner of [`crate::resolve::Greedy`], run on what
//! the player sees.
//!
//! [`plan_column`] scores each legal action by the best plan the planner tried that starts with
//! it (score of the state at the start of the next player turn against now: kills, enemy HP and
//! block, debuffs, block, Osty, HP), and marks the action the planner would take. The search runs
//! on [`Combat::blindfold_for_search`] with a fixed seed: the draw and discard piles put in card
//! order and shuffled by an independent RNG, and every RNG stream (the run's and each
//! monster's) reseeded. That copy is a function of the visible state alone, so the cards drawn,
//! random targets and damage, and the next intents rolled inside the simulation reveal nothing
//! of the real hidden state. `tests/leak.rs` checks it: positions that differ only in hidden
//! state get the same column.

use sts2_core::combat::Action;
use sts2_core::run::{Room, RoomType, Run, RunAction, RunDecision};
use sts2_core::{Combat, Status};

use crate::Resolver;
use crate::obs_v2::PLAN_BYTES;
use crate::resolve::{Snapshot, end_turn_of, resolve_choices, search_turn};

pub use crate::resolve::WIN_SCORE;

/// The seed of the blindfolded copy the planner searches. Fixed, so the column is a function
/// of the visible state.
const COLUMN_SEED: u64 = 0x524C_4E32_3243_4F4C;

/// The planner's own plan length (the full search: ~105 µs per boss-fight decision).
pub const FULL_DEPTH: usize = crate::resolve::MAX_PLAYS;

/// Plans of at most two plays: the full search's benchmark wins at ~75% of its cost.
pub const SHORT_DEPTH: usize = 2;

/// The depth of the column in the observation ([`write_plan`]): each action alone against
/// ending the turn (~30 µs per boss-fight decision, ~6% of NECRO1's collect).
pub const OBS_DEPTH: usize = 1;

/// [`PLAN_BYTES`] flags: the action has a score.
pub const PLAN_SCORED: u8 = 1;
/// The planner's choice.
pub const PLAN_CHOSEN: u8 = 2;
/// A winning play.
pub const PLAN_WINS: u8 = 4;

/// The column for one combat decision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanColumn {
    /// Per legal action (in the order given): the planner's score of the best plan that starts
    /// with it, [`WIN_SCORE`] for a play that wins; `None` if the planner could not try it.
    pub scores: Vec<Option<i64>>,
    /// The index of the action the planner takes (the first of its plan, else end turn).
    pub chosen: Option<usize>,
}

impl PlanColumn {
    /// The index of the best score (the first of equals).
    #[must_use]
    pub fn top(&self) -> Option<usize> {
        self.scores
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|s| (i, s)))
            .rev()
            .max_by_key(|&(_, s)| s)
            .map(|(i, _)| i)
    }
}

/// The column for `combat`'s current decision over `legal`.
///
/// `legal`: its legal actions, or the subset the caller models. `big_fight`: an elite or boss fight (potions are drunk sooner). `depth`: the
/// most plays a plan holds ([`FULL_DEPTH`], [`SHORT_DEPTH`], or 1 to score each action alone).
///
/// In the player's turn the planner's beam search runs once over the blindfolded copy. In a card
/// choice each option is scored by taking it, finishing the choice with the first options, and
/// ending the turn; the planner takes the first option. Anything else gets an empty column.
#[must_use]
pub fn plan_column(combat: &Combat, big_fight: bool, legal: &[Action], depth: usize) -> PlanColumn {
    column_on(combat, big_fight, legal, depth, |c| {
        c.blindfold_for_search(COLUMN_SEED)
    })
}

/// The column searched on the real state, hidden draw order and RNG included: dishonest, only
/// the leak test's control (it shows that the test can see a leak).
#[doc(hidden)]
#[must_use]
pub fn plan_column_peeking(
    combat: &Combat,
    big_fight: bool,
    legal: &[Action],
    depth: usize,
) -> PlanColumn {
    column_on(combat, big_fight, legal, depth, Combat::clone)
}

/// [`plan_column`] with the search on `world(combat)`.
fn column_on(
    combat: &Combat,
    big_fight: bool,
    legal: &[Action],
    depth: usize,
    world: impl Fn(&Combat) -> Combat,
) -> PlanColumn {
    let mut column = PlanColumn {
        scores: vec![None; legal.len()],
        chosen: None,
    };
    match combat.status() {
        Status::PlayerTurn => {
            let world = world(combat);
            let plan = search_turn(world, big_fight, legal, depth, Some(&mut column.scores));
            let want = plan.first().unwrap_or(&Action::EndTurn);
            column.chosen = legal.iter().position(|a| a == want);
        }
        Status::Decision => {
            let world = world(combat);
            let before = Snapshot::of(world.state());
            for (score, a) in column.scores.iter_mut().zip(legal) {
                let mut copy = world.clone();
                if copy.apply(a.clone()).is_ok() {
                    let after = end_turn_of(resolve_choices(copy));
                    *score = Some(before.score_to(&Snapshot::of(after.state())));
                }
            }
            column.chosen = (!legal.is_empty()).then_some(0);
        }
        _ => {}
    }
    column
}

/// Plays a fight by the column alone (RLN22's benchmark).
///
/// At every decision it recomputes the column and takes the planner's choice ([`PlanColumn::chosen`]) or, with `argmax`, the best
/// score ([`PlanColumn::top`]). No plan is kept between decisions.
#[derive(Clone, Copy, Debug)]
pub struct ColumnResolver {
    /// Take the best score instead of the planner's choice.
    pub argmax: bool,
    /// The column's search depth ([`plan_column`]).
    pub depth: usize,
}

impl Resolver for ColumnResolver {
    fn reset(&mut self, _seed: u64) {}

    fn act(&mut self, run: &Run, legal: &[RunAction]) -> RunAction {
        let RunDecision::Combat(combat) = run.pending_decision() else {
            return legal[0].clone();
        };
        let big = big_fight(run);
        let actions: Vec<Action> = legal
            .iter()
            .filter_map(|a| match a {
                RunAction::Combat(a) => Some(a.clone()),
                _ => None,
            })
            .collect();
        let column = plan_column(combat, big, &actions, self.depth);
        let pick = if self.argmax {
            column.top()
        } else {
            column.chosen
        };
        pick.map_or_else(
            || legal[0].clone(),
            |i| RunAction::Combat(actions[i].clone()),
        )
    }
}

/// Writes the observation's plan section ([`crate::obs_v2::PLAN_AT`]): [`plan_column`] at
/// [`OBS_DEPTH`].
///
/// `out` is [`crate::action::ACTIONS`] × [`PLAN_BYTES`], left as it is for the indices not
/// given; `actions` are `combat`'s decision's actions with their combat index (the indices the
/// mask holds). Only what the player sees reaches it (`tests/leak.rs`).
///
/// # Panics
/// If an index is past `out`.
pub fn write_plan(combat: &Combat, big_fight: bool, actions: &[(usize, Action)], out: &mut [u8]) {
    write_plan_at(combat, big_fight, actions, OBS_DEPTH, out);
}

/// [`write_plan`] with plans of up to `depth` plays (RLN32's `plan_depth` option: 1 is
/// [`OBS_DEPTH`], 2 is [`SHORT_DEPTH`]).
///
/// # Panics
/// If an index is past `out`.
pub fn write_plan_at(
    combat: &Combat,
    big_fight: bool,
    actions: &[(usize, Action)],
    depth: usize,
    out: &mut [u8],
) {
    let legal: Vec<Action> = actions.iter().map(|(_, a)| a.clone()).collect();
    let column = plan_column(combat, big_fight, &legal, depth);
    for (i, ((index, _), score)) in actions.iter().zip(&column.scores).enumerate() {
        let Some(score) = *score else { continue };
        let entry = &mut out[index * PLAN_BYTES..][..PLAN_BYTES];
        let shown = i16::try_from(score.clamp(-i64::from(i16::MAX), i64::from(i16::MAX)))
            .unwrap_or(i16::MAX);
        entry[..2].copy_from_slice(&shown.to_le_bytes());
        entry[2] = PLAN_SCORED
            | if column.chosen == Some(i) {
                PLAN_CHOSEN
            } else {
                0
            }
            | if score == WIN_SCORE { PLAN_WINS } else { 0 };
    }
}

/// Whether `run`'s room is an elite or boss fight (the planner drinks potions sooner there).
#[must_use]
pub const fn big_fight(run: &Run) -> bool {
    matches!(
        run.state().room,
        Room::Combat {
            room_type: RoomType::Elite | RoomType::Boss,
            ..
        }
    )
}
