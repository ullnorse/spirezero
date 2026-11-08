//! The fixed action space and the legal-action mask (`docs/specs/rl-2x-plan.md`, section 7.2).
//!
//! 140 indices: play (hand slot 0-9 × target slot 0-7, where 0 is "no target" or the player
//! itself, and 1-7 are the enemies in `enemies` order) from [`PLAY_BASE`], potion (slot 0-5 ×
//! target slot) from
//! [`POTION_BASE`], [`END_TURN`], pick option 0-9 from [`PICK_BASE`], and [`DONE`]. The mask is
//! derived from [`Combat::legal_actions_into`]'s list, never computed beside it, so the two
//! cannot disagree: each listed action sets its index's bit and records its list position, and
//! decoding an index reads that position back. A listed action with no index (a target among
//! the player's pets or past the seventh enemy, a choice option past the tenth) gets no bit and
//! is counted; so is one whose index an earlier listed action already took.

use sts2_core::Combat;
use sts2_core::combat::{Action, CreatureId, Decision, DecisionRequest};

/// Hand slots of the play block.
pub const HAND_SLOTS: usize = 10;
/// Target slots: 0 is "no target" or the player itself, 1-7 the first seven enemies in
/// `enemies` order.
pub const TARGET_SLOTS: usize = 8;
/// Potion belt slots.
pub const POTION_SLOTS: usize = 6;
/// Options of a pending card choice.
pub const OPTION_SLOTS: usize = 10;
/// The first play index: `PLAY_BASE + hand slot * TARGET_SLOTS + target slot`.
pub const PLAY_BASE: usize = 0;
/// The first potion index: `POTION_BASE + belt slot * TARGET_SLOTS + target slot`.
pub const POTION_BASE: usize = PLAY_BASE + HAND_SLOTS * TARGET_SLOTS;
/// End the turn.
pub const END_TURN: usize = POTION_BASE + POTION_SLOTS * TARGET_SLOTS;
/// The first pick index: `PICK_BASE + option`.
pub const PICK_BASE: usize = END_TURN + 1;
/// Finish a card choice ([`Decision::Done`]).
pub const DONE: usize = PICK_BASE + OPTION_SLOTS;
/// Indices in the space.
pub const ACTIONS: usize = DONE + 1;
/// `u64` words of a mask.
pub const MASK_WORDS: usize = ACTIONS.div_ceil(64);

/// The target slot of `target`: 0 for none or the player itself (a potion used on its user),
/// 1-7 for the first seven enemies.
fn target_slot(combat: &Combat, target: Option<CreatureId>) -> Option<usize> {
    let Some(target) = target else {
        return Some(0);
    };
    if target == combat.state().player.creature {
        return Some(0);
    }
    let slot = combat.state().enemies.iter().position(|&e| e == target)?;
    (slot + 1 < TARGET_SLOTS).then_some(slot + 1)
}

/// The index of `action` in `combat`'s current position, if the space has one.
#[must_use]
pub fn index_of(combat: &Combat, action: &Action) -> Option<usize> {
    match action {
        Action::PlayCard { card, target } => {
            let hand = combat.state().player.hand.iter().position(|c| c == card)?;
            (hand < HAND_SLOTS).then_some(())?;
            Some(PLAY_BASE + hand * TARGET_SLOTS + target_slot(combat, *target)?)
        }
        Action::UsePotion { slot, target } => (*slot < POTION_SLOTS)
            .then(|| target_slot(combat, *target).map(|t| POTION_BASE + slot * TARGET_SLOTS + t))?,
        Action::EndTurn => Some(END_TURN),
        Action::Resolve(Decision::Pick(card)) => {
            let Some(DecisionRequest::ChooseCards { options, .. }) = combat.pending_decision()
            else {
                return None;
            };
            let i = options.iter().position(|c| c == card)?;
            (i < OPTION_SLOTS).then_some(PICK_BASE + i)
        }
        Action::Resolve(Decision::Done) => Some(DONE),
        // The list offers a choice one pick at a time; a whole answer is never listed, nor is
        // putting a pick back (a client's).
        Action::Resolve(Decision::Cards(_) | Decision::Unpick(_)) => None,
    }
}

/// The legal actions as a bitset over a fixed space of `N` indices (`W` words), and the list
/// position of each set index. [`Mask`] is the combat space's,
/// [`RunMask`](crate::run_action::RunMask) the run space's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexMask<const N: usize, const W: usize> {
    bits: [u64; W],
    position: [u16; N],
}

/// The combat space's mask.
pub type Mask = IndexMask<ACTIONS, MASK_WORDS>;

impl<const N: usize, const W: usize> Default for IndexMask<N, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const W: usize> IndexMask<N, W> {
    /// An empty mask.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bits: [0; W],
            position: [0; N],
        }
    }

    /// The bitset: bit `i % 64` of word `i / 64` is set when index `i` is legal.
    #[must_use]
    pub const fn bits(&self) -> &[u64; W] {
        &self.bits
    }

    /// Clears every bit.
    pub const fn clear(&mut self) {
        self.bits = [0; W];
    }

    /// Fills the mask from a legal list whose `i`th action has index `index(i)`; the number of
    /// listed actions that get no bit (no index in the space, or an index an earlier action
    /// took).
    ///
    /// # Panics
    /// If the list has 65,536 actions or more.
    pub fn fill_with<T>(&mut self, legal: &[T], index: impl Fn(&T) -> Option<usize>) -> u32 {
        self.clear();
        let mut unmapped = 0;
        for (i, action) in legal.iter().enumerate() {
            match index(action) {
                Some(index) if !self.contains(index) => self.fill_one(index, i),
                _ => unmapped += 1,
            }
        }
        unmapped
    }

    /// Sets `index` at list position `position`.
    pub(crate) fn fill_one(&mut self, index: usize, position: usize) {
        self.bits[index / 64] |= 1 << (index % 64);
        self.position[index] = u16::try_from(position).expect("under 65,536 legal actions");
    }

    /// Clears `index`.
    pub(crate) const fn remove(&mut self, index: usize) {
        if index < N {
            self.bits[index / 64] &= !(1 << (index % 64));
        }
    }

    /// Whether `index` is legal.
    #[must_use]
    pub const fn contains(&self, index: usize) -> bool {
        index < N && self.bits[index / 64] & (1 << (index % 64)) != 0
    }

    /// The legal list position of a set `index`.
    #[must_use]
    pub fn position(&self, index: usize) -> Option<usize> {
        self.contains(index)
            .then(|| usize::from(self.position[index]))
    }

    /// The number of legal indices.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }

    /// The `nth` set index in index order, if there are more than `nth`.
    #[must_use]
    pub fn nth(&self, nth: u32) -> Option<usize> {
        nth_set(&self.bits, nth)
    }
}

impl Mask {
    /// Fills the mask from `legal`, `combat`'s legal list; the number of listed actions that
    /// get no bit (no index in the space, or an index an earlier action took).
    ///
    /// # Panics
    /// If the list has 65,536 actions or more.
    pub fn fill(&mut self, combat: &Combat, legal: &[Action]) -> u32 {
        self.fill_with(legal, |action| index_of(combat, action))
    }
}

/// The `nth` set bit of `bits` in index order (bit `i % 64` of word `i / 64` is index `i`).
#[must_use]
pub fn nth_set(bits: &[u64], mut nth: u32) -> Option<usize> {
    for (w, &word) in bits.iter().enumerate() {
        let ones = word.count_ones();
        if nth < ones {
            let mut word = word;
            for _ in 0..nth {
                word &= word - 1;
            }
            return Some(w * 64 + word.trailing_zeros() as usize);
        }
        nth -= ones;
    }
    None
}
