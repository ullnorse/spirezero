//! The chance script of the exhaustive solver (`docs/specs/solver.md`, section 1).
//!
//! A [`Rng`](super::Rng) marked scripted, and the draw pile while the `Shuffle` stream is scripted,
//! take their outcomes from a thread-local list of outcome indices instead of xoshiro. When the
//! list runs out, the first unanswered call is recorded as the step's [`Need`] and it, like every
//! later call, gets outcome 0; the solver discards that run and re-runs the step once per outcome.
//! Nothing here is read while no stream is scripted, which is always the case in normal play.

use std::cell::RefCell;

/// A chance event the script did not answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Need {
    /// A uniform pick among `n` outcomes (`NextInt` and what is built on it).
    Uniform(u32),
    /// A categorical pick: outcome `i` has weight `weights[i]` (`RandomBranch`,
    /// `WeightedNextItem`).
    Weights(Vec<f32>),
    /// The draw pile's top card: outcome `i` is the card at draw-pile index `i`; `keys[i]` is
    /// that card's content key ([`crate::combat::CombatState::card_key`]), so the solver can
    /// branch once per distinct card. `reactive`: some listener reacts to a drawn card or a
    /// card changing piles, so the order of draws may matter.
    ///
    /// With [batched draws](set_batch_draws) on and no listener, `count > 1` is the number of
    /// draws this draw command takes in a row from this pile (no reshuffle, hand room for all):
    /// the answer is `count` outcomes, one per draw, each a draw-pile index at that draw's
    /// moment (the picked card leaves the pile, the rest keep their order). A batch that takes
    /// the whole pile asks nothing.
    Draw {
        /// Each draw-pile card's content key.
        keys: Vec<u64>,
        /// Whether a listener reacts to draws.
        reactive: bool,
        /// The draws the answer covers (1 unless batched).
        count: u32,
    },
    /// A draw the solver cannot enumerate (a continuous roll, a known draw-pile position).
    Unsupported(&'static str),
}

/// The thread's script.
#[derive(Debug, Default)]
struct Script {
    /// Outcome indices, in call order.
    outcomes: Vec<u32>,
    /// The next one to hand out.
    pos: usize,
    /// The first call the script did not answer.
    need: Option<Need>,
    /// Whether a non-reactive draw command's draws are asked as one batch ([`Need::Draw`]).
    batch_draws: bool,
    /// Draws of the current batch still to come; they read the script without asking.
    batch_left: u32,
    /// The current batch is certain: its draws take index 0 without reading the script.
    batch_auto: bool,
}

thread_local! {
    static SCRIPT: RefCell<Script> = RefCell::new(Script::default());
}

/// Installs `outcomes` as the thread's script (position 0, no need).
pub fn install(outcomes: &[u32]) {
    SCRIPT.with_borrow_mut(|s| {
        s.outcomes.clear();
        s.outcomes.extend_from_slice(outcomes);
        s.pos = 0;
        s.need = None;
        s.batch_left = 0;
    });
}

/// Turns batched draws on or off for this thread ([`Need::Draw`]); off (every draw its own
/// event) is the brute-force check's setting.
pub fn set_batch_draws(on: bool) {
    SCRIPT.with_borrow_mut(|s| s.batch_draws = on);
}

/// Takes the need the last run recorded, if any.
#[must_use]
pub fn take_need() -> Option<Need> {
    SCRIPT.with_borrow_mut(|s| s.need.take())
}

/// The script position (a step's start, for [`rewind`]).
#[must_use]
pub(crate) fn mark() -> usize {
    SCRIPT.with_borrow(|s| s.pos)
}

/// Goes back to `mark` for a re-run of the same step; a need recorded since is dropped.
pub(crate) fn rewind(mark: usize) {
    SCRIPT.with_borrow_mut(|s| {
        s.pos = mark;
        s.need = None;
        s.batch_left = 0;
    });
}

/// The outcomes consumed since `mark`.
#[must_use]
pub(crate) fn consumed_since(mark: usize) -> Vec<u32> {
    SCRIPT.with_borrow(|s| s.outcomes[mark.min(s.pos)..s.pos].to_vec())
}

/// Puts `prefix` back in front of the unread part of the script: a step resumed after a
/// player decision replays the outcomes it consumed before the prompt.
pub(crate) fn resume(prefix: &[u32]) {
    SCRIPT.with_borrow_mut(|s| {
        let rest = s.outcomes.split_off(s.pos);
        s.outcomes.clear();
        s.outcomes.extend_from_slice(prefix);
        s.outcomes.extend_from_slice(&rest);
        s.pos = 0;
    });
}

/// The next outcome, or 0 after recording `need()` when the script has run out. Answers are
/// clamped below `bound` so that a stale script never indexes out of range.
fn next(bound: u32, need: impl FnOnce() -> Need) -> u32 {
    SCRIPT.with_borrow_mut(|s| {
        if s.batch_left > 0 {
            s.batch_left = 0;
            if s.need.is_none() {
                s.need = Some(Need::Unsupported("a chance event inside a draw batch"));
            }
        }
        if s.need.is_none()
            && let Some(&o) = s.outcomes.get(s.pos)
        {
            s.pos += 1;
            return o.min(bound.saturating_sub(1));
        }
        if s.need.is_none() {
            s.need = Some(need());
        }
        0
    })
}

/// A uniform pick in `0..n` (`n >= 1`).
pub(crate) fn uniform(n: u32) -> u32 {
    next(n, || Need::Uniform(n))
}

/// A categorical pick by `weights`.
pub(crate) fn weighted(weights: &[f32]) -> usize {
    let bound = u32::try_from(weights.len()).unwrap_or(u32::MAX);
    next(bound, || Need::Weights(weights.to_vec())) as usize
}

/// The draw-pile index of the next card drawn, from a pile of `len` cards (`len >= 1`).
/// `count` is how many draws the draw command takes in a row from this pile, `reactive`
/// whether a listener reacts to them, and `keys` gives the pile's content keys; see
/// [`Need::Draw`].
pub(crate) fn draw(
    len: usize,
    count: u32,
    reactive: bool,
    keys: impl FnOnce() -> Vec<u64>,
) -> usize {
    let bound = u32::try_from(len).unwrap_or(u32::MAX);
    let batch = SCRIPT.with_borrow_mut(|s| {
        if s.batch_left == 0 {
            return None;
        }
        s.batch_left -= 1;
        if s.batch_auto || s.need.is_some() {
            return Some(0);
        }
        let o = s.outcomes.get(s.pos).copied();
        s.pos += 1;
        Some(o.map_or_else(
            || {
                s.need = Some(Need::Unsupported("a draw batch shorter than its draws"));
                0
            },
            |o| o.min(bound.saturating_sub(1)),
        ))
    });
    if let Some(index) = batch {
        return index as usize;
    }
    let batched = !reactive && count > 1 && SCRIPT.with_borrow(|s| s.batch_draws);
    if !batched {
        if len < 2 {
            return 0;
        }
        return next(bound, || Need::Draw {
            keys: keys(),
            reactive,
            count: 1,
        }) as usize;
    }
    if count as usize >= len {
        // The whole pile: certain.
        SCRIPT.with_borrow_mut(|s| {
            s.batch_left = count - 1;
            s.batch_auto = true;
        });
        return 0;
    }
    let index = next(bound, || Need::Draw {
        keys: keys(),
        reactive,
        count,
    });
    SCRIPT.with_borrow_mut(|s| {
        if s.need.is_none() {
            s.batch_left = count - 1;
            s.batch_auto = false;
        }
    });
    index as usize
}

/// The end of a draw command: a batch it did not finish is an error the solver reports.
pub(crate) fn end_draws() {
    SCRIPT.with_borrow_mut(|s| {
        if s.batch_left > 0 {
            s.batch_left = 0;
            if s.need.is_none() {
                s.need = Some(Need::Unsupported("a draw batch cut short"));
            }
        }
    });
}

/// Records a draw the solver cannot enumerate; the caller goes on with its default.
pub(crate) fn unsupported(what: &'static str) {
    SCRIPT.with_borrow_mut(|s| {
        if s.need.is_none() {
            s.need = Some(Need::Unsupported(what));
        }
    });
}
