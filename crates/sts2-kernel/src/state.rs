//! The fight state: plain `Copy` data with fixed capacities, no heap.
//!
//! Ids are indices, as in sts2-core: a card's id is its index in [`Fight::cards`] (the same as
//! sts2-core's `CardId`), a creature's its index in [`Fight::creatures`] (`CreatureId`). Each
//! combat pile is a fixed array ([`Pile`]); the draw pile is stored bottom first, so drawing from
//! its top is a pop. Capacities are fixed; a fight that would outgrow one stops as
//! [`Status::Unsupported`].

use sts2_core::rng::{Rng, RngState};

use crate::tables::{Affliction, P};

/// Cards per fight (deck plus every generated card).
pub const MAX_CARDS: usize = 120;
/// Creatures per fight (the player plus every enemy ever created).
pub const MAX_CREATURES: usize = 8;
/// Powers per creature.
pub const MAX_POWERS: usize = 6;
/// Hand size (`CardPile.MaxCardsInHand`).
pub const MAX_HAND: usize = 10;

/// "No creature / card / slot".
pub const NONE: u8 = u8::MAX;
/// The player's creature id.
pub const PLAYER: u8 = 0;

/// The combat piles, in listener order (`PileType.LISTENER_ORDER`).
pub mod pile {
    pub const HAND: u8 = 0;
    pub const DRAW: u8 = 1;
    pub const DISCARD: u8 = 2;
    pub const EXHAUST: u8 = 3;
    pub const PLAY: u8 = 4;
}

/// Where a fight stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Status {
    #[default]
    PlayerTurn,
    /// A card choice is pending (`Survivor`'s discard).
    Decision,
    Won,
    Lost,
    /// The fight reached something the kernel does not support (a capacity, or a number outside
    /// the exact fixed-point domain); no action is accepted.
    Unsupported,
}

/// A player input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Action {
    /// Listed first so that an all-zero buffer of actions is cheap to make.
    #[default]
    EndTurn,
    /// Play the hand card `card` at `target` ([`NONE`] for an untargeted card).
    Play { card: u8, target: u8 },
    /// Pick a card of the pending choice.
    Pick(u8),
}

/// A power instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Power {
    pub kind: P,
    /// `SkipNextDurationTick`.
    pub skip_tick: bool,
    /// `Applier`, or [`NONE`].
    pub applier: u8,
    /// Identity within the fight (dispatch snapshots refer to instances by it).
    pub uid: u16,
    pub amount: i32,
    /// `SlowPower.SlowAmount`; `IllusionPower.IsReviving` and
    /// `RitualPower._wasJustAppliedByEnemy` (0 or 1).
    pub data: i32,
}

/// `MonsterModel.SetMoveImmediate`'s forced moves the kernel supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Forced {
    #[default]
    None,
    /// `Creature.StunInternal`'s `STUNNED`.
    Stunned,
    /// `IllusionPower`'s `REVIVE_MOVE`.
    Revive,
}

/// A monster's per-combat state (`MonsterModel` and its `MoveMachine`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent `MonsterModel` or `MoveMachine` field"
)]
pub struct Monster {
    /// Index into [`crate::Kernel::monsters`].
    pub kind: u8,
    /// The current table state.
    pub current: u8,
    /// A forced move replacing the current state. Every supported one must be performed once
    /// (`MustPerformOnceBeforeTransitioning`).
    pub forced: Forced,
    pub forced_follow_up: u8,
    /// `_performedAtLeastOnce` of the forced move.
    pub forced_performed: bool,
    /// `_performedFirstMove`.
    pub performed_first_move: bool,
    /// `SpawnedThisTurn`.
    pub spawned_this_turn: bool,
    /// `IsPerformingMove`.
    pub performing: bool,
    /// `StateLog.Count`, saturating.
    pub log_len: u8,
    /// The last three logged states, most recent first.
    pub recent: [u8; 3],
    /// Every state ever logged, as bits.
    pub used: u16,
    /// Per-kind fields that conditions read (`Nibbit.IsFront`, `Toadpole.IsFront`).
    pub aux: u8,
}

/// A creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Creature {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub enemy: bool,
    pub n_powers: u8,
    /// Index into the encounter's slots, or [`NONE`].
    pub slot: u8,
    /// The kinds in `powers`, as [`P::bit`]s.
    pub mask: u32,
    pub powers: [Power; MAX_POWERS],
    /// Unused for the player.
    pub monster: Monster,
}

impl Creature {
    /// `IsAlive`.
    #[must_use]
    pub const fn alive(&self) -> bool {
        self.hp > 0
    }

    /// The powers, in application order.
    #[must_use]
    pub fn powers(&self) -> &[Power] {
        &self.powers[..usize::from(self.n_powers)]
    }

    /// Whether the creature has a power of kind `p`.
    #[must_use]
    pub const fn has(&self, p: P) -> bool {
        self.mask & p.bit() != 0
    }

    /// The first power of kind `p`.
    #[must_use]
    pub fn find(&self, p: P) -> Option<usize> {
        if !self.has(p) {
            return None;
        }
        self.powers().iter().position(|q| q.kind == p)
    }

    /// The amount of the power of kind `p`, or 0.
    #[must_use]
    pub fn amount(&self, p: P) -> i32 {
        self.find(p).map_or(0, |i| self.powers[i].amount)
    }

    /// The power with `uid`.
    #[must_use]
    pub fn by_uid(&self, uid: u16) -> Option<usize> {
        self.powers().iter().position(|q| q.uid == uid)
    }

    /// `IsSecondaryEnemy`: an enemy with a secondary power (`MinionPower`).
    #[must_use]
    pub const fn secondary(&self) -> bool {
        self.enemy && self.has(P::Minion)
    }

    /// `IsPrimaryEnemy`.
    #[must_use]
    pub const fn primary_enemy(&self) -> bool {
        self.enemy && !self.has(P::Minion)
    }

    /// Recomputes [`Creature::mask`].
    pub fn remask(&mut self) {
        self.mask = self.powers().iter().fold(0, |m, p| m | p.kind.bit());
    }
}

/// A combat card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Card {
    /// Index into [`crate::Kernel::cards`].
    pub spec: u8,
    /// The pile holding it, or [`NONE`].
    pub pile: u8,
    pub affliction: Affliction,
}

/// A pending choice: the `Survivor` in the Play pile waiting for its discard.
pub type Pending = u8;

/// A pile of card ids, in storage order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pile<const N: usize> {
    len: u8,
    cards: [u8; N],
}

impl<const N: usize> Pile<N> {
    /// An empty pile.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: 0,
            cards: [0; N],
        }
    }

    /// The cards, in storage order.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.cards[..usize::from(self.len)]
    }

    /// How many cards.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether it is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts `card` at `index` (storage order).
    ///
    /// # Panics
    /// If the pile is full (the callers bound every pile by [`MAX_CARDS`] or [`MAX_HAND`]).
    pub fn insert(&mut self, index: usize, card: u8) {
        let len = self.len();
        assert!(len < N, "pile overflow");
        self.cards.copy_within(index..len, index + 1);
        self.cards[index] = card;
        self.len += 1;
    }

    /// Appends `card`.
    ///
    /// # Panics
    /// If the pile is full.
    pub fn push(&mut self, card: u8) {
        let len = self.len();
        assert!(len < N, "pile overflow");
        self.cards[len] = card;
        self.len += 1;
    }

    /// Removes and returns the last card.
    pub const fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.cards[self.len as usize])
    }

    /// Removes `card`; whether it was there.
    pub fn remove(&mut self, card: u8) -> bool {
        let len = self.len();
        let Some(at) = self.cards[..len].iter().position(|&c| c == card) else {
            return false;
        };
        self.cards.copy_within(at + 1..len, at);
        self.len -= 1;
        true
    }

    /// Empties the pile.
    pub const fn clear(&mut self) {
        self.len = 0;
    }

    /// The pile with its unused slots zeroed.
    #[must_use]
    pub fn canonical(&self) -> Self {
        let mut p = *self;
        p.cards[self.len()..].fill(0);
        p
    }
}

impl<const N: usize> Default for Pile<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// One fight. `Copy`: clone it by assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent `CombatState` field"
)]
pub struct Fight {
    /// `Rng.Shuffle`.
    pub rng_shuffle: RngState,
    /// `Rng.MonsterAi`.
    pub rng_ai: RngState,
    /// `Rng.Niche` (summoned monsters' HP).
    pub rng_niche: RngState,
    pub energy: i32,
    pub max_energy: i32,
    /// `RoundNumber`.
    pub round: u32,
    /// `PlayerCombatState.TurnNumber`.
    pub turn: u32,
    pub status: Status,
    pub ascension: u8,
    /// Index into [`crate::Kernel::encounters`].
    pub encounter: u8,
    /// [`crate::tables::relic`] bits.
    pub relics: u8,
    pub enemy_side: bool,
    /// `CombatManager.IsInProgress`.
    pub in_progress: bool,
    /// `PendingLoss != null`.
    pub pending_loss: bool,
    /// `Player.IsActiveForHooks`: cleared when the player dies.
    pub hooks_active: bool,
    /// Whether the player started a card play this turn (`CombatHistory`'s `CardPlayStarted`
    /// this turn, which `RingingPower` reads).
    pub played_this_turn: bool,
    /// The `Survivor` waiting for its discard, or [`NONE`].
    pub pending: Pending,
    pub n_creatures: u8,
    pub n_enemies: u8,
    pub n_cards: u8,
    pub next_uid: u16,
    /// `Enemies`, in slot order.
    pub enemies: [u8; MAX_CREATURES - 1],
    pub hand: Pile<MAX_HAND>,
    /// The Play pile (a card being played; a turn-end card resolving).
    pub play: Pile<MAX_HAND>,
    /// Stored bottom first: the top card is the last.
    pub draw: Pile<MAX_CARDS>,
    pub discard: Pile<MAX_CARDS>,
    pub exhaust: Pile<MAX_CARDS>,
    pub creatures: [Creature; MAX_CREATURES],
    pub cards: [Card; MAX_CARDS],
}

impl Fight {
    /// The cards of pile `p` in pile order (the draw pile from its top), as sts2-core lists them.
    pub fn pile_cards(&self, p: u8) -> impl Iterator<Item = u8> + '_ {
        let (cards, reversed): (&[u8], bool) = match p {
            pile::HAND => (self.hand.as_slice(), false),
            pile::DRAW => (self.draw.as_slice(), true),
            pile::DISCARD => (self.discard.as_slice(), false),
            pile::EXHAUST => (self.exhaust.as_slice(), false),
            _ => (self.play.as_slice(), false),
        };
        let n = cards.len();
        (0..n).map(move |i| cards[if reversed { n - 1 - i } else { i }])
    }

    /// The number of cards in pile `p`.
    #[must_use]
    pub const fn pile_len(&self, p: u8) -> usize {
        match p {
            pile::HAND => self.hand.len(),
            pile::DRAW => self.draw.len(),
            pile::DISCARD => self.discard.len(),
            pile::EXHAUST => self.exhaust.len(),
            _ => self.play.len(),
        }
    }

    /// The hand, in order.
    #[must_use]
    pub fn hand(&self) -> &[u8] {
        self.hand.as_slice()
    }

    /// The top card of the draw pile.
    #[must_use]
    pub fn draw_top(&self) -> Option<u8> {
        self.draw.as_slice().last().copied()
    }

    /// Removes `card` from its pile, if any, and returns the pile.
    ///
    /// # Panics
    /// If the card's pile does not hold it (the piles and [`Card::pile`] are out of step).
    pub fn remove_from_pile(&mut self, card: u8) -> Option<u8> {
        let p = self.cards[usize::from(card)].pile;
        if p == NONE {
            return None;
        }
        let removed = match p {
            pile::HAND => self.hand.remove(card),
            pile::DRAW => self.draw.remove(card),
            pile::DISCARD => self.discard.remove(card),
            pile::EXHAUST => self.exhaust.remove(card),
            _ => self.play.remove(card),
        };
        assert!(removed, "a card is in its pile");
        self.cards[usize::from(card)].pile = NONE;
        Some(p)
    }

    /// Puts `card` (in no pile) at the bottom of pile `p`.
    pub fn insert_at_bottom(&mut self, card: u8, p: u8) {
        debug_assert_eq!(self.cards[usize::from(card)].pile, NONE);
        match p {
            pile::HAND => self.hand.push(card),
            pile::DRAW => self.draw.insert(0, card),
            pile::DISCARD => self.discard.push(card),
            pile::EXHAUST => self.exhaust.push(card),
            _ => self.play.push(card),
        }
        self.cards[usize::from(card)].pile = p;
    }

    /// The enemies, in order.
    #[must_use]
    pub fn enemies(&self) -> &[u8] {
        &self.enemies[..usize::from(self.n_enemies)]
    }

    /// The creature `id`.
    #[must_use]
    pub fn creature(&self, id: u8) -> &Creature {
        &self.creatures[usize::from(id)]
    }

    /// The creature `id`, mutably.
    pub fn creature_mut(&mut self, id: u8) -> &mut Creature {
        &mut self.creatures[usize::from(id)]
    }

    /// The card `id`.
    #[must_use]
    pub fn card(&self, id: u8) -> &Card {
        &self.cards[usize::from(id)]
    }

    /// The player creature.
    #[must_use]
    pub fn player(&self) -> &Creature {
        &self.creatures[usize::from(PLAYER)]
    }

    /// `ContainsCreature`: the player (the only ally) or an enemy in combat.
    #[must_use]
    pub fn contains(&self, id: u8) -> bool {
        id == PLAYER || self.enemies().contains(&id)
    }

    /// The same fight with power uids renumbered in creature order and the unused array slots
    /// cleared, so that two fights that differ only in how their powers were numbered or in
    /// stale slots compare equal.
    #[must_use]
    pub fn canonical(&self) -> Self {
        let mut f = *self;
        let mut uid = 0;
        for c in &mut f.creatures[..usize::from(self.n_creatures)] {
            let n = usize::from(c.n_powers);
            for p in &mut c.powers[..n] {
                p.uid = uid;
                uid += 1;
            }
            c.powers[n..].fill(Power::default());
        }
        f.creatures[usize::from(self.n_creatures)..].fill(Creature::default());
        f.enemies[usize::from(self.n_enemies)..].fill(0);
        f.cards[usize::from(self.n_cards)..].fill(Card::default());
        f.hand = self.hand.canonical();
        f.play = self.play.canonical();
        f.draw = self.draw.canonical();
        f.discard = self.discard.canonical();
        f.exhaust = self.exhaust.canonical();
        f.next_uid = uid;
        f
    }

    /// An FNV-1a digest of the canonical state (deterministic, for the differential test and
    /// for search tables).
    #[must_use]
    pub fn digest(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = Fnv(0xCBF2_9CE4_8422_2325);
        self.canonical().hash(&mut h);
        h.finish()
    }
}

/// FNV-1a as a [`std::hash::Hasher`].
struct Fnv(u64);

impl std::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }
}

/// Runs `f` on the stream `state` as an sts2-core [`Rng`] and stores the advanced state back.
pub fn with_rng<T>(state: &mut RngState, f: impl FnOnce(&mut Rng) -> T) -> T {
    let mut rng = Rng::from_state(*state);
    let out = f(&mut rng);
    *state = rng.state();
    out
}
