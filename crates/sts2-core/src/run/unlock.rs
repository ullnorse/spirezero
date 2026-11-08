//! Profile state that changes which content a run can see (`Unlocks/UnlockState.cs`).

use std::collections::BTreeSet;

use crate::content::characters::Character;

/// A set of ids that is either everything or an explicit subset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum IdSet {
    /// Every id (the game builds this from `EpochModel.AllEpochIds` / `ModelDb.AllEncounters`).
    #[default]
    All,
    /// Exactly these ids.
    Only(BTreeSet<String>),
}

impl IdSet {
    /// Whether `id` is in the set.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(ids) => ids.contains(id),
        }
    }
}

/// The player's unlock state, passed in explicitly because no save file is read.
///
/// `Default` is the referee's state: `UnlockState.all`, every epoch revealed, every encounter
/// seen and `NumberOfRuns = 999999999`, so no first-run override applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlockState {
    /// Revealed epoch ids; they filter event pools and card/relic/potion pools.
    pub epochs_revealed: IdSet,
    /// Encounter ids seen (won or lost).
    pub encounters_seen: IdSet,
    /// Number of runs started; `0` switches on the scripted first-run behaviour.
    pub number_of_runs: i32,
}

impl UnlockState {
    /// `UnlockState.all`'s run count.
    pub const ALL_NUMBER_OF_RUNS: i32 = 999_999_999;

    /// `UnlockState.all`: the referee's state.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            epochs_revealed: IdSet::All,
            encounters_seen: IdSet::All,
            number_of_runs: Self::ALL_NUMBER_OF_RUNS,
        }
    }

    /// `UnlockState.none`: a brand-new profile (first run).
    #[must_use]
    pub const fn none() -> Self {
        Self {
            epochs_revealed: IdSet::Only(BTreeSet::new()),
            encounters_seen: IdSet::Only(BTreeSet::new()),
            number_of_runs: 0,
        }
    }

    /// `UnlockState.Characters` (`Unlocks/UnlockState.cs:57-80`): `ModelDb.AllCharacters` minus
    /// Silent, Regent, Necrobinder and Defect while their first epoch is not revealed.
    #[must_use]
    pub fn characters(&self) -> Vec<Character> {
        Character::MODEL_DB_ORDER
            .into_iter()
            .filter(|&c| {
                let epoch = match c {
                    Character::Ironclad => return true,
                    Character::Silent => "SILENT1_EPOCH",
                    Character::Regent => "REGENT1_EPOCH",
                    Character::Necrobinder => "NECROBINDER1_EPOCH",
                    Character::Defect => "DEFECT1_EPOCH",
                };
                self.epochs_revealed.contains(epoch)
            })
            .collect()
    }

    /// Whether this is the profile's first run (`NumberOfRuns == 0`).
    #[must_use]
    pub const fn is_first_run(&self) -> bool {
        self.number_of_runs == 0
    }
}

impl Default for UnlockState {
    fn default() -> Self {
        Self::all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_referee_state() {
        let state = UnlockState::default();
        assert!(!state.is_first_run());
        assert!(state.epochs_revealed.contains("ANY_EPOCH"));
        assert_eq!(state.number_of_runs, 999_999_999);
    }

    #[test]
    fn none_is_a_first_run() {
        let state = UnlockState::none();
        assert!(state.is_first_run());
        assert!(!state.encounters_seen.contains("ANY_ENCOUNTER"));
    }
}
