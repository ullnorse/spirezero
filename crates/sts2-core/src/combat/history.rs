//! `CombatHistory` (`Combat.History/CombatHistory.cs`).
//!
//! The append-only log of combat events that game logic queries ("cards drawn this turn",
//! "Ethereal cards played"). Entries are
//! logged right after their event, before its `AfterX` hook, and cleared when combat is won
//! (`CombatManager.cs:1316`, between `AfterCombatEnd` and `AfterCombatVictory`).
//!
//! The log is indexed, not scanned: this turn's entries are kept in one list with a count per
//! kind, and earlier turns survive only as the combat-wide counts game logic reads (every
//! ported query is "this turn" or a count). A new query over earlier turns needs a new index
//! here.
//!
//! Only the entries a ported model reads are recorded; `CardDiscarded` (no `CardCmd.Discard`
//! yet), `PotionUsed`,
//! `MonsterPerformedMove`, `CardAfflicted` and `Summoned` are not.

use super::card_play::CardPlay;
use super::commands::damage::DamageResult;
use super::engine::Engine;
use super::engine::ValueProps;
use super::state::{CardId, CombatState, CreatureId, OrbKind, Side};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::CardType;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// One `CombatHistoryEntry`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HistoryEntry {
    /// `Actor`.
    pub actor: CreatureId,
    /// `RoundNumber` when it happened.
    pub round: u32,
    /// `CurrentSide` when it happened.
    pub side: Side,
    /// The player's `TurnNumber` when it happened.
    pub turn_number: u32,
    /// What happened.
    pub kind: HistoryKind,
}

/// The entry types (`Combat.History.Entries`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HistoryKind {
    /// `CardPlayStartedEntry`.
    CardPlayStarted(CardPlay),
    /// `CardPlayFinishedEntry`. `card_type` and `was_ethereal` are read when the entry is
    /// made (`WasEthereal`, `CardPlayFinishedEntry.cs`).
    CardPlayFinished {
        /// The play.
        play: CardPlay,
        /// The card's type when the entry was made.
        card_type: CardType,
        /// `WasEthereal`.
        was_ethereal: bool,
    },
    /// `CardDrawnEntry`.
    /// `Card` and `FromHandDraw`.
    CardDrawn {
        /// The card drawn.
        card: CardId,
        /// `FromHandDraw`: drawn by the turn's hand draw.
        from_hand_draw: bool,
    },
    /// `CardExhaustedEntry`.
    CardExhausted(CardId),
    /// `CardAfflictedEntry`.
    CardAfflicted {
        /// The card.
        card: CardId,
        /// The affliction.
        affliction: AfflictionKind,
    },
    /// `CardDiscardedEntry`.
    CardDiscarded(CardId),
    /// `CardGeneratedEntry`; `creator` is the creating player's creature.
    CardGenerated {
        /// The generated card.
        card: CardId,
        /// The creating player's creature.
        creator: Option<CreatureId>,
    },
    /// `EnergySpentEntry`.
    EnergySpent(i32),
    /// `BlockGainedEntry` (the actor is the receiver): the gain truncated, its props and the
    /// `CardPlay` that caused it (`CreatureCmd.cs:686`).
    BlockGained {
        /// The Block gained, truncated.
        amount: i32,
        /// Its props.
        props: ValueProps,
        /// The play that caused it.
        play: Option<CardPlay>,
    },
    /// `CreatureAttackedEntry` (the actor is the attacker).
    CreatureAttacked,
    /// `DamageReceivedEntry` (the actor is the receiver): one per `DamageResult`, `props` being
    /// the `Result.Props` of the damage.
    DamageReceived {
        /// The damage.
        result: DamageResult,
        /// `Result.Props`.
        props: ValueProps,
        /// `Dealer`.
        dealer: Option<CreatureId>,
        /// `CardSource`.
        card_source: Option<CardId>,
    },
    /// `OrbChanneledEntry` (the actor is the orb's owner).
    OrbChanneled(OrbKind),
    /// `PowerReceivedEntry` (the actor is the power's owner).
    PowerReceived {
        /// The power.
        kind: PowerKind,
        /// The amount received.
        amount: Decimal,
        /// `Applier`.
        applier: Option<CreatureId>,
    },
    /// `StarsModifiedEntry` (the actor is the player): the change of `Stars`, never 0.
    StarsModified(i32),
}

impl HistoryKind {
    /// The entry type without its data.
    #[must_use]
    pub const fn tag(&self) -> HistoryTag {
        match self {
            Self::CardPlayStarted(_) => HistoryTag::CardPlayStarted,
            Self::CardPlayFinished { .. } => HistoryTag::CardPlayFinished,
            Self::CardDrawn { .. } => HistoryTag::CardDrawn,
            Self::CardExhausted(_) => HistoryTag::CardExhausted,
            Self::CardAfflicted { .. } => HistoryTag::CardAfflicted,
            Self::CardDiscarded(_) => HistoryTag::CardDiscarded,
            Self::CardGenerated { .. } => HistoryTag::CardGenerated,
            Self::EnergySpent(_) => HistoryTag::EnergySpent,
            Self::BlockGained { .. } => HistoryTag::BlockGained,
            Self::CreatureAttacked => HistoryTag::CreatureAttacked,
            Self::DamageReceived { .. } => HistoryTag::DamageReceived,
            Self::OrbChanneled(_) => HistoryTag::OrbChanneled,
            Self::PowerReceived { .. } => HistoryTag::PowerReceived,
            Self::StarsModified(_) => HistoryTag::StarsModified,
        }
    }
}

/// A [`HistoryKind`] without its data, for indexed queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HistoryTag {
    /// [`HistoryKind::CardPlayStarted`].
    CardPlayStarted,
    /// [`HistoryKind::CardPlayFinished`].
    CardPlayFinished,
    /// [`HistoryKind::CardDrawn`].
    CardDrawn,
    /// [`HistoryKind::CardExhausted`].
    CardExhausted,
    /// [`HistoryKind::CardAfflicted`].
    CardAfflicted,
    /// [`HistoryKind::CardDiscarded`].
    CardDiscarded,
    /// [`HistoryKind::CardGenerated`].
    CardGenerated,
    /// [`HistoryKind::EnergySpent`].
    EnergySpent,
    /// [`HistoryKind::BlockGained`].
    BlockGained,
    /// [`HistoryKind::CreatureAttacked`].
    CreatureAttacked,
    /// [`HistoryKind::DamageReceived`].
    DamageReceived,
    /// [`HistoryKind::OrbChanneled`].
    OrbChanneled,
    /// [`HistoryKind::PowerReceived`].
    PowerReceived,
    /// [`HistoryKind::StarsModified`].
    StarsModified,
}

impl_index!(HistoryTag);

impl HistoryTag {
    /// The number of tags.
    const COUNT: usize = 14;
}

/// `HappenedThisTurn`'s key: round, side and the player's turn number.
type TurnKey = (u32, Side, u32);

/// `CombatHistory`, indexed (see the module docs).
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct CombatHistory {
    /// The turn `this_turn` belongs to; `None` before the first entry.
    turn: Option<TurnKey>,
    /// That turn's entries, oldest first: one list, so a clone (a search copy) copies one.
    this_turn: Vec<HistoryEntry>,
    /// That turn's entries by [`HistoryTag`].
    this_turn_counts: [u32; HistoryTag::COUNT],
    /// Entries by [`HistoryTag`] over the whole combat.
    counts: [u32; HistoryTag::COUNT],
    /// The player's finished Ethereal card plays over the whole combat.
    ethereal_plays_finished: u32,
    /// `DamageReceived` entries with `Result.UnblockedDamage > 0` over the whole combat, per
    /// receiver.
    hp_loss_hits: Vec<(CreatureId, u32)>,
    /// The player's `OrbChanneled` entries over the whole combat, per [`OrbKind`] (in
    /// [`OrbKind::ALL`] order).
    orbs_channeled: [u32; OrbKind::ALL.len()],
    /// The `CardGenerated` entries created by the player over the whole combat.
    generated_by_player: u32,
    /// The card of the player's last finished non-dupe Attack play of the latest player turn
    /// number with one, and of the one before (`HistoryCourse`), as `(turn number, card)`.
    last_attacks: [Option<(u32, CardId)>; 2],
}

/// Room a cloned turn list keeps for new entries: a clone (a search copy) records more of the
/// turn, and a list cloned at its exact length would reallocate on its next entry.
const CLONE_SPARE: usize = 8;

impl Clone for CombatHistory {
    fn clone(&self) -> Self {
        let mut this_turn = Vec::new();
        if !self.this_turn.is_empty() {
            this_turn.reserve_exact(self.this_turn.len() + CLONE_SPARE);
            this_turn.extend_from_slice(&self.this_turn);
        }
        Self {
            turn: self.turn,
            this_turn,
            this_turn_counts: self.this_turn_counts,
            counts: self.counts,
            ethereal_plays_finished: self.ethereal_plays_finished,
            hp_loss_hits: self.hp_loss_hits.clone(),
            orbs_channeled: self.orbs_channeled,
            generated_by_player: self.generated_by_player,
            last_attacks: self.last_attacks,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let Self {
            turn,
            this_turn,
            this_turn_counts,
            counts,
            ethereal_plays_finished,
            hp_loss_hits,
            orbs_channeled,
            generated_by_player,
            last_attacks,
        } = source;
        self.turn = *turn;
        crate::copy_vec_from(&mut self.this_turn, this_turn);
        self.this_turn_counts = *this_turn_counts;
        self.counts = *counts;
        self.ethereal_plays_finished = *ethereal_plays_finished;
        crate::copy_vec_from(&mut self.hp_loss_hits, hp_loss_hits);
        self.orbs_channeled = *orbs_channeled;
        self.generated_by_player = *generated_by_player;
        self.last_attacks = *last_attacks;
    }
}

impl CombatHistory {
    /// `CombatHistory.Clear`. The lists keep their buffers: a search copy that ends the
    /// combat would otherwise drop them, and the next copy into it allocate them again.
    pub(crate) fn clear(&mut self) {
        let Self {
            turn,
            this_turn,
            this_turn_counts,
            counts,
            ethereal_plays_finished,
            hp_loss_hits,
            orbs_channeled,
            generated_by_player,
            last_attacks,
        } = self;
        *turn = None;
        this_turn.clear();
        *this_turn_counts = [0; HistoryTag::COUNT];
        *counts = [0; HistoryTag::COUNT];
        *ethereal_plays_finished = 0;
        hp_loss_hits.clear();
        *orbs_channeled = [0; OrbKind::ALL.len()];
        *generated_by_player = 0;
        *last_attacks = [None; 2];
    }

    /// Starts the buckets of the turn `(round, side, turn_number)`. Turn keys only move forward, so the old turn is
    /// never queried again.
    #[cold]
    #[inline(never)]
    fn start_turn(&mut self, round: u32, side: Side, turn_number: u32) {
        // Passed as scalars: a key built on the stack just before is reloaded with a wider
        // load than its stores (a store-forwarding stall).
        self.turn = Some((round, side, turn_number));
        self.this_turn.clear();
        self.this_turn_counts = [0; HistoryTag::COUNT];
    }

    /// Inlined into each [`Engine::record`] site, so the entry is written straight into its
    /// bucket from registers: built on the stack and copied, its narrow field stores stall the
    /// wide loads of the copy (failed store forwarding).
    #[expect(
        clippy::inline_always,
        reason = "measured: without it, sites copy the entry through the stack"
    )]
    #[inline(always)]
    fn push(&mut self, entry: HistoryEntry, player: CreatureId) {
        let key = (entry.round, entry.side, entry.turn_number);
        if self.turn != Some(key) {
            self.start_turn(entry.round, entry.side, entry.turn_number);
        }
        let tag = entry.kind.tag().index();
        self.counts[tag] += 1;
        if entry.actor == player
            && matches!(
                entry.kind,
                HistoryKind::CardPlayFinished {
                    was_ethereal: true,
                    ..
                }
            )
        {
            self.ethereal_plays_finished += 1;
        }
        if let HistoryKind::DamageReceived { result, .. } = entry.kind
            && result.unblocked > 0
        {
            match self
                .hp_loss_hits
                .iter_mut()
                .find(|(c, _)| *c == entry.actor)
            {
                Some((_, hits)) => *hits += 1,
                None => self.hp_loss_hits.push((entry.actor, 1)),
            }
        }
        if let HistoryKind::OrbChanneled(kind) = entry.kind
            && entry.actor == player
        {
            self.orbs_channeled[kind.index()] += 1;
        }
        if let HistoryKind::CardGenerated { creator, .. } = entry.kind
            && creator == Some(player)
        {
            self.generated_by_player += 1;
        }
        self.this_turn_counts[tag] += 1;
        self.this_turn.push(entry);
    }
}

/// The entries of one [`HistoryTag`] that happened this turn ([`CombatState::history_this_turn`]).
#[derive(Clone, Copy, Debug)]
pub struct TurnEntries<'a> {
    /// This turn's entries of every tag.
    entries: &'a [HistoryEntry],
    tag: HistoryTag,
    len: usize,
}

impl<'a> TurnEntries<'a> {
    /// The entries, oldest first.
    pub fn iter(self) -> impl DoubleEndedIterator<Item = &'a HistoryEntry> + Clone {
        let entries = if self.len == 0 { &[] } else { self.entries };
        let tag = self.tag;
        entries.iter().filter(move |e| e.kind.tag() == tag)
    }

    /// The number of entries.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

impl CombatState {
    /// The `tag` entries that happened this turn (`HappenedThisTurn`), oldest first.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        reason = "a `u32` count; `usize` is at least 32 bits on every supported target"
    )]
    pub fn history_this_turn(&self, tag: HistoryTag) -> TurnEntries<'_> {
        let key = (self.round, self.side, self.player.turn_number);
        let (entries, len) = if self.history.turn == Some(key) {
            let len = self.history.this_turn_counts[tag.index()];
            (&self.history.this_turn[..], len as usize)
        } else {
            (&[][..], 0)
        };
        TurnEntries { entries, tag, len }
    }

    /// A mark for [`CombatState::history_since`]: how many entries this turn has so far.
    pub(crate) fn history_mark(&self) -> usize {
        let key = (self.round, self.side, self.player.turn_number);
        if self.history.turn == Some(key) {
            self.history.this_turn.len()
        } else {
            0
        }
    }

    /// The entries recorded since [`CombatState::history_mark`] returned `mark`, oldest first,
    /// if the turn has not changed in between (else every entry of the current turn).
    pub(crate) fn history_since(&self, mark: usize) -> &[HistoryEntry] {
        let key = (self.round, self.side, self.player.turn_number);
        if self.history.turn == Some(key) {
            self.history.this_turn.get(mark..).unwrap_or_default()
        } else {
            &[]
        }
    }

    /// The number of `tag` entries this combat.
    #[must_use]
    pub const fn history_count(&self, tag: HistoryTag) -> u32 {
        self.history.counts[tag.index()]
    }

    /// Whether `creature` lost HP to damage this turn: any `DamageReceivedEntry` this turn with
    /// that receiver and `Result.UnblockedDamage > 0` (`Spite.LostHpThisTurn`).
    #[must_use]
    pub fn lost_hp_this_turn(&self, creature: CreatureId) -> bool {
        self.history_this_turn(HistoryTag::DamageReceived)
            .iter()
            .any(|e| {
                matches!(e.kind, HistoryKind::DamageReceived { result, .. } if result.unblocked > 0)
                    && e.actor == creature
            })
    }

    /// The `DamageReceivedEntry`s over the whole combat with `creature` as the receiver and
    /// `Result.UnblockedDamage > 0` (`TearAsunder`'s hit count minus 1).
    #[must_use]
    pub fn hp_loss_hits_this_combat(&self, creature: CreatureId) -> u32 {
        self.history
            .hp_loss_hits
            .iter()
            .find_map(|&(c, hits)| (c == creature).then_some(hits))
            .unwrap_or(0)
    }

    /// The `CardGeneratedEntry`s whose `Creator` is the player, this combat (`Supermassive`).
    #[must_use]
    pub const fn cards_generated_by_player_this_combat(&self) -> u32 {
        self.history.generated_by_player
    }

    /// The `OrbChanneledEntry`s of `kind` with the player as the actor, this combat
    /// (`Voltaic`'s `CalculatedChannels` for Lightning).
    #[must_use]
    pub const fn orbs_channeled_this_combat(&self, kind: OrbKind) -> u32 {
        self.history.orbs_channeled[kind.index()]
    }

    /// The card of the player's last finished non-dupe Attack play whose entry has the player's
    /// turn number `turn` (`CardPlaysFinished.LastOrDefault` with `HappenedLastPlayerTurn` for
    /// `turn` = `TurnNumber - 1`, `HistoryCourse`).
    #[must_use]
    pub fn last_attack_played_in_turn(&self, turn: u32) -> Option<CardId> {
        self.history
            .last_attacks
            .iter()
            .flatten()
            .find_map(|&(t, card)| (t == turn).then_some(card))
    }

    /// The player's finished card plays that were Ethereal (`CardPlaysFinished` with
    /// `CardPlay.Player == Owner` and `WasEthereal`), this combat.
    #[must_use]
    pub const fn ethereal_card_plays_finished(&self) -> u32 {
        self.history.ethereal_plays_finished
    }
}

impl Engine<'_> {
    /// `CombatHistory.Add`: append an entry stamped with the current round, side and turn.
    /// `IsLiveCombat` is always true for the real combat state.
    /// Inlined, like [`CombatHistory::push`].
    #[expect(
        clippy::inline_always,
        reason = "measured: without it, sites copy the entry through the stack"
    )]
    #[inline(always)]
    pub(crate) fn record(&mut self, actor: CreatureId, kind: HistoryKind) {
        let entry = HistoryEntry {
            actor,
            round: self.s.round,
            side: self.s.side,
            turn_number: self.s.player.turn_number,
            kind,
        };
        let player = self.s.player.creature;
        if let HistoryKind::CardPlayFinished {
            play,
            card_type: CardType::Attack,
            ..
        } = entry.kind
            && actor == player
            && !self.s.card(play.card).is_dupe
        {
            let last = &mut self.s.history.last_attacks;
            let turn = entry.turn_number;
            if last[0].is_some_and(|(t, _)| t != turn) {
                last[1] = last[0];
            }
            last[0] = Some((turn, play.card));
        }
        self.s.history.push(entry, player);
    }
}
