//! How [`Run::apply`] stays atomic without copying the run for every action.
//!
//! An action outside combat can fail after it has changed the run (an `Unsupported*` error deep
//! in a room, a map the game would throw on), and a rejected action must leave the run exactly as
//! it was. Instead of applying each such action to a copy, the run keeps a [`Journal`]: a
//! checkpoint (a copy of the run, taken now and then) and every action applied since. A rejected
//! action restores the checkpoint and replays the log; the engine is deterministic, so that is
//! the run as it was before the action. Rejections are rare (an unported card in a selection), so
//! the copy is paid once per [`CHECKPOINT_EVERY`] actions instead of once per action.
//!
//! Every way to change a run outside [`Run::apply`] (the loadout and debug hooks, the builders)
//! drops the journal ([`Run::forget_journal`]), so the next action takes a fresh checkpoint;
//! so must a unit test that changes the run's fields directly.
//! Debug builds also apply every action outside combat on a copy and check that a rejection
//! restored exactly that copy.

use std::fmt;

use super::{Run, RunAction, RunError};

/// The longest log: the most actions a rejection replays.
const CHECKPOINT_EVERY: usize = 64;

/// The rollback state of a [`Run`]: not part of the run, so it is not copied
/// ([`Clone`] gives an empty journal), compared or printed.
#[derive(Default)]
pub(super) struct Journal {
    /// The run at some earlier point, with an empty journal.
    checkpoint: Option<Box<Run>>,
    /// The actions applied since the checkpoint, in order.
    log: Vec<RunAction>,
}

impl Clone for Journal {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for Journal {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Journal {}

impl fmt::Debug for Journal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Journal")
    }
}

impl Run {
    /// Applies an action that may fail after changing the run; on failure the run is restored.
    pub(super) fn apply_atomic(&mut self, action: RunAction) -> Result<(), RunError> {
        let mut journal = std::mem::take(&mut self.journal);
        if journal.checkpoint.is_none() || journal.log.len() >= CHECKPOINT_EVERY {
            journal.checkpoint = Some(Box::new(self.clone()));
            journal.log.clear();
        }
        #[cfg(debug_assertions)]
        let before = self.clone();
        match self.apply_unjournaled(&action) {
            Ok(()) => {
                journal.log.push(action);
                self.journal = journal;
                Ok(())
            }
            Err(e) => {
                self.journal = journal;
                self.restore();
                #[cfg(debug_assertions)]
                assert!(
                    *self == before,
                    "the replay restores the run (a change outside `Run::apply` must \
                     `forget_journal`)"
                );
                Err(e)
            }
        }
    }

    /// Applies an action that changes nothing when it is rejected: a potion discard, or an
    /// action in the current combat. The combat action that wins the fight goes on to the
    /// victory, which can fail after changing the run (a relic's victory hook meets content
    /// that is not ported); then the run is restored from the journal, which a combat always
    /// has (entering it took a checkpoint, else the first action here takes one).
    pub(super) fn apply_in_place(&mut self, action: RunAction) -> Result<(), RunError> {
        let in_combat = matches!(action, RunAction::Combat(_));
        if in_combat && self.journal.checkpoint.is_none() {
            self.journal.checkpoint = Some(Box::new(self.clone()));
        }
        match self.apply_unjournaled(&action) {
            Ok(()) => {
                self.record(action);
                Ok(())
            }
            // `Combat::apply` rejected it: nothing changed.
            Err(e @ RunError::Combat(_)) => Err(e),
            Err(e) => {
                if in_combat {
                    self.restore();
                }
                Err(e)
            }
        }
    }

    /// Puts the run back where the journal's log ends: its checkpoint with every logged action
    /// replayed.
    fn restore(&mut self) {
        let journal = std::mem::take(&mut self.journal);
        let mut run = journal
            .checkpoint
            .as_deref()
            .expect("a checkpoint was taken")
            .clone();
        for action in &journal.log {
            run.apply_unjournaled(action).expect(
                "an applied action replays (a change outside `Run::apply` must \
                 `forget_journal`)",
            );
        }
        *self = run;
        self.journal = journal;
    }

    /// Notes an action that was applied in place (in combat, a potion discard).
    pub(super) fn record(&mut self, action: RunAction) {
        if self.journal.checkpoint.is_some() {
            self.journal.log.push(action);
        }
    }

    /// The number of actions a rejection would replay.
    #[cfg(test)]
    pub(super) const fn journal_len(&self) -> usize {
        self.journal.log.len()
    }

    /// Drops the journal after a change outside [`Run::apply`].
    pub(super) fn forget_journal(&mut self) {
        self.journal = Journal::default();
    }
}
