//! `MonsterMoveStateMachine` (`MonsterMoves.MonsterMoveStateMachine/*.cs`) as static tables plus
//! plain-data progress.
//!
//! A monster describes its machine once as a [`MoveTable`] of [`StateDef`]s (what
//! `GenerateMoveStateMachine` builds); the per-combat part is a [`MoveMachine`]: the current
//! state, whether a move was performed, and the log of moves used for repeat and cooldown rules.

use crate::combat::{CombatState, CreatureId};
use crate::rng::Rng;
use smallvec::SmallVec;

/// An index into a [`MoveTable`].
pub type StateIdx = usize;

/// `MoveRepeatType` of a random branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// `Repeat.CanRepeatForever`.
    CanRepeatForever,
    /// `CanRepeatXTimes` with its `maxTimes`.
    CanRepeatXTimes(u32),
    /// `Repeat.CannotRepeat`.
    CannotRepeat,
    /// `Repeat.UseOnlyOnce`.
    UseOnlyOnce,
}

/// One `RandomBranchState.StateWeight`.
#[derive(Clone, Copy, Debug)]
pub struct Branch {
    /// The state it leads to.
    pub state: StateIdx,
    /// `RepeatType`.
    pub repeat: Repeat,
    /// `Cooldown`.
    pub cooldown: u32,
    /// The weight lambda's value when it is a constant.
    pub weight: f32,
    /// A weight lambda that reads the combat (`TwoTailedRat`); overrides [`Branch::weight`].
    pub weight_fn: Option<WeightFn>,
}

/// A weight lambda (`Func<float>`) of a branch, evaluated on the owner when the branch is rolled.
pub type WeightFn = fn(&CombatState, CreatureId) -> f32;

impl Branch {
    /// `AddBranch(state, repeatType)`: weight 1, no cooldown.
    #[must_use]
    pub const fn new(state: StateIdx, repeat: Repeat) -> Self {
        Self {
            state,
            repeat,
            cooldown: 0,
            weight: 1.0,
            weight_fn: None,
        }
    }

    /// `AddBranch(state, repeatType, weight)`: a constant weight, no cooldown.
    #[must_use]
    pub const fn weighted(state: StateIdx, repeat: Repeat, weight: f32) -> Self {
        Self {
            state,
            repeat,
            cooldown: 0,
            weight,
            weight_fn: None,
        }
    }

    /// `AddBranch(state, cooldown, repeatType, Func<float> weight)`: a weight read from the combat.
    #[must_use]
    pub const fn dynamic(
        state: StateIdx,
        cooldown: u32,
        repeat: Repeat,
        weight_fn: WeightFn,
    ) -> Self {
        Self {
            state,
            repeat,
            cooldown,
            weight: 1.0,
            weight_fn: Some(weight_fn),
        }
    }
}

/// A condition of a `ConditionalBranchState`, evaluated on the owner.
pub type Condition = fn(&CombatState, CreatureId) -> bool;

/// A `ConditionalBranchState` branch: `state` when `condition` holds (the call coerces the
/// function to a [`Condition`]).
#[must_use]
pub const fn when(state: StateIdx, condition: Condition) -> (StateIdx, Condition) {
    (state, condition)
}

/// A `MonsterState`.
#[derive(Clone, Copy, Debug)]
pub enum StateDef {
    /// `MoveState`: performs a move, then `FollowUpState`.
    Move {
        /// `StateId`.
        id: &'static str,
        /// `FollowUpState`.
        follow_up: StateIdx,
    },
    /// `RandomBranchState`: weighted pick on `Rng.MonsterAi`.
    Random {
        /// `StateId`.
        id: &'static str,
        /// The weighted branches.
        branches: &'static [Branch],
    },
    /// `ConditionalBranchState`: the first branch whose condition holds.
    Conditional {
        /// `StateId`.
        id: &'static str,
        /// Each branch's state and condition.
        branches: &'static [(StateIdx, Condition)],
    },
}

impl StateDef {
    /// `MonsterState.Id`.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Move { id, .. } | Self::Random { id, .. } | Self::Conditional { id, .. } => id,
        }
    }

    /// `IsMove`; also `ShouldAppearInLogs` (branch states never appear in the log).
    #[must_use]
    pub const fn is_move(&self) -> bool {
        matches!(self, Self::Move { .. })
    }
}

/// What `GenerateMoveStateMachine` returns: the states and the initial one.
#[derive(Clone, Copy, Debug)]
pub struct MoveTable {
    /// The states.
    pub states: &'static [StateDef],
    /// The initial state.
    pub initial: StateIdx,
}

/// A move built outside the monster's table and set with `SetMoveImmediate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ForcedMove {
    /// `IllusionPower`'s `REVIVE_MOVE` (`IllusionPower.cs:91-101`).
    IllusionRevive,
    /// `Creature.StunInternal`'s `STUNNED` move: does nothing.
    Stunned,
    /// `Creature.StunInternal`'s `STUNNED` move around `SlumberingBeetle.WakeUpMove`, which
    /// removes the beetle's `Plating`.
    BeetleWakeUp,
    /// `WaterfallGiant`'s `ABOUT_TO_BLOW_MOVE` (`MustPerformOnceBeforeTransitioning`), which only
    /// `SteamEruptionPower` enters, by `SetMoveImmediate(.., forceTransition: true)`.
    AboutToBlow,
    /// `TestSubject`'s dead state `RESPAWN_MOVE` (`MustPerformOnceBeforeTransitioning`), which
    /// only `AdaptablePower` enters, by `SetMoveImmediate(.., forceTransition: true)`.
    TestSubjectRespawn,
}

impl ForcedMove {
    /// `MoveState.Id`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::IllusionRevive => "REVIVE_MOVE",
            Self::Stunned | Self::BeetleWakeUp => "STUNNED",
            Self::AboutToBlow => "ABOUT_TO_BLOW_MOVE",
            Self::TestSubjectRespawn => "RESPAWN_MOVE",
        }
    }
}

/// The current state when it is a [`ForcedMove`] (`MonsterModel.SetMoveImmediate`,
/// `MoveStateMachine.ForceCurrentState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Forced {
    /// The forced move.
    pub kind: ForcedMove,
    /// `FollowUpStateId`.
    pub follow_up: StateIdx,
    /// `MustPerformOnceBeforeTransitioning`.
    pub must_perform_once: bool,
    /// `_performedAtLeastOnce`, cleared when the state is left.
    pub performed_at_least_once: bool,
}

/// `StateLog` as the rules read it: its length, last entries and the states it ever held.
///
/// No repeat or cooldown rule looks further back than [`MoveLog::TAIL`] entries,
/// and no table has more states than the set holds ([`MoveLog::fits`], checked for every
/// table a machine is built for), so this answers every query the full log would.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MoveLog {
    /// The number of entries.
    len: u32,
    /// The last `min(len, TAIL)` entries, oldest first.
    tail: [u8; Self::TAIL],
    /// Bit `s` is set if state `s` was ever logged.
    used: u128,
}

impl MoveLog {
    /// Entries kept: the longest look-back of a rule (`CanRepeatXTimes(2)`, cooldown 3).
    pub const TAIL: usize = 3;

    /// The empty log.
    pub const EMPTY: Self = Self {
        len: 0,
        tail: [0; Self::TAIL],
        used: 0,
    };

    /// Whether every rule of `table` reads at most [`MoveLog::TAIL`] entries back and every
    /// state fits the set.
    #[must_use]
    pub fn fits(table: &MoveTable) -> bool {
        let reach = |b: &Branch| match b.repeat {
            Repeat::CanRepeatXTimes(n) => usize::try_from(n).is_ok_and(|n| n <= Self::TAIL),
            Repeat::CanRepeatForever | Repeat::CannotRepeat | Repeat::UseOnlyOnce => true,
        } && usize::try_from(b.cooldown).is_ok_and(|n| n <= Self::TAIL);
        table.states.len() <= 128
            && table.states.iter().all(|s| match s {
                StateDef::Random { branches, .. } => branches.iter().all(reach),
                StateDef::Move { .. } | StateDef::Conditional { .. } => true,
            })
    }

    /// The log `entries`, oldest first.
    #[must_use]
    pub fn from_slice(entries: &[StateIdx]) -> Self {
        let mut log = Self::EMPTY;
        for &s in entries {
            log.push(s);
        }
        log
    }

    /// Appends `state`.
    ///
    /// # Panics
    /// If `state` is past 127 (no table has that many states, [`MoveLog::fits`]).
    pub fn push(&mut self, state: StateIdx) {
        let s = u8::try_from(state).expect("a move table has at most 128 states");
        self.tail.rotate_left(1);
        self.tail[Self::TAIL - 1] = s;
        self.used |= 1 << state;
        self.len += 1;
    }

    /// The number of entries.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "const fn: `From` is not const")]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the log is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The last entry.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "const fn: `From` is not const")]
    pub const fn last(&self) -> Option<StateIdx> {
        if self.len == 0 {
            None
        } else {
            Some(self.tail[Self::TAIL - 1] as StateIdx)
        }
    }

    /// The last `min(n, len)` entries, newest first.
    ///
    /// # Panics
    /// If `n` is past [`MoveLog::TAIL`].
    pub fn recent(&self, n: usize) -> impl Iterator<Item = StateIdx> + '_ {
        assert!(n <= Self::TAIL, "the log keeps {} entries", Self::TAIL);
        self.tail
            .iter()
            .rev()
            .take(n.min(self.len()))
            .map(|&s| StateIdx::from(s))
    }

    /// Whether `state` was ever logged.
    #[must_use]
    pub const fn contains(&self, state: StateIdx) -> bool {
        state < 128 && self.used & (1 << state) != 0
    }

    /// Whether the log holds at least `n` entries and its last `n` are all `state`.
    #[must_use]
    pub fn ends_with_run(&self, state: StateIdx, n: u32) -> bool {
        let n = usize::try_from(n).unwrap_or(usize::MAX);
        self.len() >= n && self.recent(n).all(|s| s == state)
    }
}

/// The per-combat progress of a monster's machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MoveMachine {
    /// The current table state; ignored while [`MoveMachine::forced`] is set.
    pub current: StateIdx,
    /// A forced move that replaced the current state, if any.
    pub forced: Option<Forced>,
    /// `_performedFirstMove`.
    pub performed_first_move: bool,
    /// `StateLog`: every move the machine entered, the initial one included.
    pub log: MoveLog,
}

impl MoveMachine {
    /// The machine as constructed: at the initial state, logged if it is a move.
    #[must_use]
    ///
    /// # Panics
    /// If the table's rules look back further than [`MoveLog`] keeps.
    pub fn new(table: &MoveTable) -> Self {
        assert!(MoveLog::fits(table), "a move table outgrows MoveLog");
        let mut log = MoveLog::EMPTY;
        if table.states[table.initial].is_move() {
            log.push(table.initial);
        }
        Self {
            current: table.initial,
            forced: None,
            performed_first_move: false,
            log,
        }
    }

    /// `CanTransitionAway` of the current state: false for a forced move that must be
    /// performed once and was not yet. No table state sets `MustPerformOnceBeforeTransitioning`.
    #[must_use]
    pub fn can_transition_away(&self) -> bool {
        self.forced
            .is_none_or(|f| !f.must_perform_once || f.performed_at_least_once)
    }

    /// The id of the current state (`NextMove.Id` once a move was rolled).
    #[must_use]
    pub fn current_id(&self, table: &MoveTable) -> &'static str {
        self.forced
            .map_or_else(|| table.states[self.current].id(), |f| f.kind.id())
    }

    /// `RollMove` -> `FindNextMoveState` (`MonsterMoveStateMachine.cs:53-90`). No transition
    /// away from a state that cannot transition away, or from a move before the first move was
    /// performed; otherwise follow states until a move is reached and log it.
    pub fn roll(
        &mut self,
        table: &MoveTable,
        state: &CombatState,
        owner: CreatureId,
        rng: &mut Rng,
    ) {
        if self.rolls(table) {
            let next = self.next_move(table, state, owner, rng);
            self.enter(next);
        }
    }

    /// Whether [`MoveMachine::roll`] transitions at all (without it, the roll changes nothing
    /// and draws nothing).
    #[must_use]
    pub fn rolls(&self, table: &MoveTable) -> bool {
        let current_is_move = self.forced.is_some() || table.states[self.current].is_move();
        self.can_transition_away() && (self.performed_first_move || !current_is_move)
    }

    /// The move [`MoveMachine::roll`] reaches when it [`MoveMachine::rolls`]: states are
    /// followed until a move. Only reads the machine: the walk's position is local, and the log
    /// is not touched until [`MoveMachine::enter`].
    pub fn next_move(
        &self,
        table: &MoveTable,
        state: &CombatState,
        owner: CreatureId,
        rng: &mut Rng,
    ) -> StateIdx {
        let (mut current, mut forced) = (self.current, self.forced);
        loop {
            current = self.next_state(current, forced, table, state, owner, rng);
            // SetCurrentState: leaving a forced move drops it (`OnExitState`).
            forced = None;
            if table.states[current].is_move() {
                return current;
            }
        }
    }

    /// Enters the move `next` that a roll reached: it becomes current, a forced move is dropped,
    /// and it is logged.
    pub fn enter(&mut self, next: StateIdx) {
        self.current = next;
        self.forced = None;
        self.log.push(next);
    }

    /// `SetMoveImmediate(state)` (`MonsterModel.cs:421-433`): replace the current state with a
    /// forced move if the current one can transition away. Not logged.
    pub fn set_move_immediate(
        &mut self,
        kind: ForcedMove,
        follow_up: StateIdx,
        must_perform_once: bool,
    ) {
        if self.can_transition_away() {
            self.forced = Some(Forced {
                kind,
                follow_up,
                must_perform_once,
                performed_at_least_once: false,
            });
        }
    }

    /// `SetMoveImmediate(state, forceTransition: true)`: replaces the current state even if it
    /// cannot transition away. Not logged.
    pub const fn force_current_move(
        &mut self,
        kind: ForcedMove,
        follow_up: StateIdx,
        must_perform_once: bool,
    ) {
        self.forced = Some(Forced {
            kind,
            follow_up,
            must_perform_once,
            performed_at_least_once: false,
        });
    }

    /// `GetNextState` of the current state.
    fn next_state(
        &self,
        current: StateIdx,
        forced: Option<Forced>,
        table: &MoveTable,
        state: &CombatState,
        owner: CreatureId,
        rng: &mut Rng,
    ) -> StateIdx {
        if let Some(forced) = forced {
            return forced.follow_up;
        }
        match table.states[current] {
            StateDef::Move { follow_up, .. } => follow_up,
            StateDef::Conditional { id, branches } => branches
                .iter()
                .find(|(_, condition)| condition(state, owner))
                .map_or_else(|| panic!("{id}: no valid next state"), |&(s, _)| s),
            StateDef::Random { id, branches } => {
                let weights: SmallVec<[f32; 8]> = branches
                    .iter()
                    .map(|b| {
                        if self.allows(b) {
                            b.weight_fn.map_or(b.weight, |f| f(state, owner))
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let pick = rng
                    .random_branch(&weights)
                    .unwrap_or_else(|| panic!("{id}: no valid state"));
                branches[pick].state
            }
        }
    }

    /// Whether the repeat and cooldown rules let `branch` be picked (`GetStateWeight` is
    /// `weight` if so, else 0; `RandomBranchState.cs:150-185`).
    fn allows(&self, branch: &Branch) -> bool {
        let log = &self.log;
        let allowed = match branch.repeat {
            Repeat::CanRepeatForever => true,
            Repeat::UseOnlyOnce => !log.contains(branch.state),
            Repeat::CannotRepeat => !log.ends_with_run(branch.state, 1),
            Repeat::CanRepeatXTimes(max) => !log.ends_with_run(branch.state, max),
        };
        let cooling_down = branch.cooldown > 0
            && log
                .recent(usize::try_from(branch.cooldown).unwrap_or(usize::MAX))
                .any(|s| s == branch.state);
        allowed && !cooling_down
    }
}

#[cfg(test)]
mod tests {
    use super::{Branch, MoveLog, MoveMachine, Repeat};

    fn machine(log: &[usize]) -> MoveMachine {
        MoveMachine {
            current: 0,
            forced: None,
            performed_first_move: true,
            log: MoveLog::from_slice(log),
        }
    }

    #[test]
    fn repeat_rules_follow_get_state_weight() {
        // RandomBranchState.cs:150-185.
        let cannot = Branch::new(1, Repeat::CannotRepeat);
        assert!(machine(&[]).allows(&cannot));
        assert!(machine(&[1, 0]).allows(&cannot));
        assert!(!machine(&[0, 1]).allows(&cannot));

        let twice = Branch::new(1, Repeat::CanRepeatXTimes(2));
        assert!(machine(&[1]).allows(&twice));
        assert!(machine(&[1, 0, 1]).allows(&twice));
        assert!(!machine(&[0, 1, 1]).allows(&twice));

        let once = Branch::new(1, Repeat::UseOnlyOnce);
        assert!(machine(&[0, 0]).allows(&once));
        assert!(!machine(&[1, 0]).allows(&once));

        let cooldown = Branch {
            cooldown: 2,
            ..Branch::new(1, Repeat::CanRepeatForever)
        };
        assert!(!machine(&[1, 0]).allows(&cooldown));
        assert!(machine(&[1, 0, 0]).allows(&cooldown));
    }
}
