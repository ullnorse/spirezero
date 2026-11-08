//! The combat engine.
//!
//! Execution model C (`docs/decisions.md`): effects are plain nested calls mirroring the
//! game's `await` chains. A player choice with no recorded answer aborts the action; the
//! choice is exposed as a [`DecisionRequest`], and answering it restores the pre-action
//! snapshot and replays the action with the decision log.

mod card_play;
pub(crate) mod commands;
pub mod cost_lists;
pub mod energy_cost;
mod engine;
pub mod history;
pub(crate) mod hooks;
mod intent;
mod listeners;
mod potion_use;
mod preview;
pub mod star_cost;
pub mod state;
mod turn;

use std::fmt;

use crate::shuffle::unstable_shuffle;
pub use card_play::{AutoPlayType, CardPlay, ResultPile};
pub(crate) use card_play::{Resources, ResultLocation};
use engine::Interrupt;
pub use engine::{
    ContentKind, Decision, DecisionRequest, Event, Prompt, PromptSource, PromptText, Unsupported,
    ValueProps,
};
pub(crate) use engine::{Engine, Flow};
use history::CombatHistory;
pub use history::{HistoryEntry, HistoryKind, HistoryTag, TurnEntries};
pub use hooks::Hooked;
use hooks::{Hook, HookSet};
pub use intent::{AttackIntent, Intent, IntentScratch, IntentType};
pub use preview::{
    CreatureOutcome, EnemyIncoming, IncomingPreview, PlayOutcome, Preventer, UnplayableReason,
};
pub use state::{
    CardId, CombatResult, CombatState, Creature, CreatureId, Creatures, ExtraReward, Orb, OrbId,
    OrbKind, PileType, Potion, PotionId, PowerId, RelicId, Side, TurnPhase,
};

use crate::content::cards::{CardKeyword, CardKind, TargetType};
use crate::content::characters::Character;
use crate::content::enchantments::Enchantment;
pub use crate::content::encounters::Encounter;
use crate::content::encounters::EncounterData;
use crate::content::monsters::{Monster, MonsterKind};
use crate::content::potions::{PotionKind, PotionUsage};
use crate::content::relics::OwnedRelic;
use crate::rng::{Rng, RunRngSet, RunRngType, canonicalize_seed};
use crate::run::RoomType;
use crate::run::odds::GoldProportion;
use cost_lists::CostLists;
use state::{Card, Player, PowerKinds, Relic};

/// A card in the run deck: model, upgrade level, enchantment and saved counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeckCard {
    /// The card model.
    pub kind: CardKind,
    /// Times upgraded.
    pub upgrade_level: u8,
    /// The deck card's `Enchantment`; the combat clone starts with it.
    pub enchantment: Option<Enchantment>,
    /// The deck card's saved counter ([`RunCard::counter`](crate::run::RunCard)); the combat
    /// clone starts with the fields it stands for, and a combat card can change its deck
    /// version's (`DeckVersion`, `TheScythe`). Read it back from [`CombatState`]'s `deck` after
    /// the fight.
    pub counter: u32,
}

impl DeckCard {
    /// An unupgraded card.
    #[must_use]
    pub const fn new(kind: CardKind) -> Self {
        Self {
            kind,
            upgrade_level: 0,
            enchantment: None,
            counter: 0,
        }
    }

    /// An upgraded card.
    #[must_use]
    pub const fn upgraded(kind: CardKind) -> Self {
        Self {
            kind,
            upgrade_level: 1,
            enchantment: None,
            counter: 0,
        }
    }
}

/// A potion in a belt slot at combat start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BeltPotion {
    /// A potion the engine ports.
    Ported(PotionKind),
    /// A potion it does not (`ModelId.Entry`): it holds its slot and never acts.
    Unported(&'static str),
}

impl From<PotionKind> for BeltPotion {
    fn from(kind: PotionKind) -> Self {
        Self::Ported(kind)
    }
}

/// Everything needed to set up a combat. No save-file state is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatConfig {
    /// `Player.Character`: decides the orb slots and the pools combat effects draw from.
    pub character: Character,
    /// The run's RNG streams as of combat start. A fresh run's streams come from
    /// [`CombatConfig::new`]; pass advanced streams for later combats.
    pub rng: RunRngSet,
    /// The ascension level.
    pub ascension: u8,
    /// The encounter fought.
    pub encounter: Encounter,
    /// `CurrentRoom.RoomType` (`CombatRoom.RoomType` is `Encounter.RoomType`).
    pub room_type: RoomType,
    /// The run deck, in deck order (it feeds the initial shuffle).
    pub deck: Vec<DeckCard>,
    /// The relics in acquisition order, with their fields as of combat start; read them back
    /// from [`CombatState`]'s `player.relics` after the fight.
    pub relics: Vec<OwnedRelic>,
    /// `MaxPotionCount`: the number of belt slots.
    pub potion_slots: usize,
    /// The belt by slot (`None` is an empty slot); at most `potion_slots` long, the slots past
    /// its end are empty. [`CombatConfig::with_potions`] fills the first slots.
    pub potions: Vec<Option<BeltPotion>>,
    /// Current HP at combat start.
    pub player_hp: i32,
    /// Max HP at combat start.
    pub player_max_hp: i32,
    /// `MaxEnergy` at combat start.
    pub max_energy: i32,
    /// `Player.Gold` at combat start.
    pub gold: i32,
    /// Where the run is (`RunState.TotalFloor`, `CurrentActIndex`, `CurrentMapCoord` as
    /// `(col, row)`); they seed the encounter's and each monster's `Rng`.
    pub total_floor: i32,
    /// `CurrentActIndex`.
    pub act_index: i32,
    /// `CurrentMapCoord` as `(col, row)`.
    pub map_coord: Option<(i32, i32)>,
    /// Record [`Event`]s ([`Combat::events`]). Off by default: the log is for viewers and
    /// tests, and costs time.
    pub record_events: bool,
    /// The run's modifiers that act inside a fight.
    pub modifiers: crate::run::modifiers::CombatModifiers,
}

impl CombatConfig {
    /// `character` with its starter deck and relic, its starting max HP (healed at Neow: in
    /// full, or less from `WearyTraveler` on), energy and gold, and an empty belt of 3 slots (2
    /// from `TightBelt` on), on fresh streams from `seed` (canonicalized like the game does),
    /// facing `FuzzyWurmCrawlerWeak`, at the run position the referee's forced `enter_room` has
    /// right after `start_run`.
    ///
    /// # Errors
    /// The first starter card the engine does not port.
    pub fn new(character: Character, seed: &str, ascension: u8) -> Result<Self, Unsupported> {
        let mut deck = character
            .starting_deck()
            .iter()
            .map(|&id| {
                CardKind::ALL
                    .into_iter()
                    .find(|k| k.entry() == id)
                    .map(DeckCard::new)
                    .ok_or_else(|| Unsupported::card(id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if ascension >= crate::run::ASCENDERS_BANE {
            // `AscendersBane.ApplyEffectsTo`: the curse joins the end of the starting deck.
            deck.push(DeckCard::new(CardKind::AscendersBane));
        }
        let max_hp = character.starting_hp();
        Ok(Self {
            character,
            rng: RunRngSet::new(&canonicalize_seed(seed)),
            ascension,
            encounter: Encounter::FuzzyWurmCrawlerWeak,
            room_type: RoomType::Monster,
            deck,
            relics: vec![character.starting_relic().into()],
            potion_slots: if ascension >= TIGHT_BELT {
                INITIAL_POTION_SLOTS - 1
            } else {
                INITIAL_POTION_SLOTS
            },
            potions: Vec::new(),
            player_hp: crate::run::neow::hp_after_neow(max_hp, ascension),
            player_max_hp: max_hp,
            max_energy: character.max_energy(),
            gold: character.starting_gold(),
            total_floor: REFEREE_TOTAL_FLOOR,
            act_index: 0,
            map_coord: None,
            record_events: false,
            modifiers: crate::run::modifiers::CombatModifiers::default(),
        })
    }

    /// [`CombatConfig::new`] for a Necrobinder (66 max HP, 3 energy, 99 gold).
    ///
    /// # Panics
    /// Never: every Necrobinder starter card is ported.
    #[must_use]
    pub fn necrobinder(seed: &str, ascension: u8) -> Self {
        Self::new(Character::Necrobinder, seed, ascension)
            .expect("every Necrobinder starter card is ported")
    }

    /// The same, recording events.
    #[must_use]
    pub const fn with_events(mut self) -> Self {
        self.record_events = true;
        self
    }

    /// The same, with `potions` in the first belt slots.
    #[must_use]
    pub fn with_potions(mut self, potions: &[PotionKind]) -> Self {
        self.potions = potions.iter().map(|&k| Some(k.into())).collect();
        self
    }

    /// The same, facing `encounter`.
    ///
    /// # Panics
    /// If `encounter` is in no act's encounter list (every ported encounter is).
    #[must_use]
    pub fn with_encounter(mut self, encounter: Encounter) -> Self {
        self.encounter = encounter;
        self.room_type = crate::run::rooms::room_type_of_encounter(encounter.entry())
            .expect("every ported encounter is in an act's list");
        self
    }
}

/// `Player.initialMaxPotionSlotCount`.
const INITIAL_POTION_SLOTS: usize = 3;
/// `AscensionLevel.TightBelt`: one potion slot fewer (`AscensionManager.ApplyEffectsTo`).
const TIGHT_BELT: u8 = 4;

/// `TotalFloor` when the referee forces a room right after `start_run`: the Neow map point is
/// already in the history. Found by matching `SlimesWeak` compositions over seeds `D1S0..5`
/// (`docs/semantics.md`, C2); `CurrentMapCoord` there is unknown (no in-scope consumer).
const REFEREE_TOTAL_FLOOR: i32 = 1;

/// A player input.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Play the card `card` from the hand (`PlayCardAction`).
    PlayCard {
        /// The card, in the hand.
        card: CardId,
        /// Its target: an enemy for an `AnyEnemy` card, else `None`.
        target: Option<CreatureId>,
    },
    /// Use the potion in belt slot `slot` (`UsePotionAction`).
    UsePotion {
        /// The belt slot.
        slot: usize,
        /// Its target, or `None` (the owner, if valid, else no target).
        target: Option<CreatureId>,
    },
    /// End the turn (`EndTurnAction`).
    EndTurn,
    /// Answer the pending [`DecisionRequest`].
    Resolve(Decision),
}

/// Where the combat stands after an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    /// The player's play phase: play a card, use a potion or end the turn.
    PlayerTurn,
    /// A [`DecisionRequest`] is pending.
    Decision,
    /// The combat was won.
    Won,
    /// The combat was lost.
    Lost,
    /// The combat stopped at content the engine does not port (for example a generated card):
    /// the state is as the game's was when it met it, and no action is accepted any more.
    Unsupported(Unsupported),
}

/// Why an action was rejected. A rejected action changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionError {
    /// The combat is over.
    CombatOver,
    /// A decision is pending: only [`Action::Resolve`] is accepted.
    DecisionPending,
    /// No decision is pending.
    NoDecisionPending,
    /// The decision does not answer the pending request.
    InvalidDecision,
    /// The card is not in the hand.
    CardNotInHand,
    /// The card has the Unplayable keyword or a listener's `ShouldPlay` vetoes it.
    Unplayable,
    /// Not enough energy for the card's cost.
    NotEnoughEnergy,
    /// Not enough stars for the card's star cost.
    NotEnoughStars,
    /// The target is not valid for the card or potion.
    InvalidTarget,
    /// The belt slot is empty.
    NoPotionInSlot,
    /// The potion cannot be used now (`Automatic`, or the owner is dead).
    PotionNotUsable,
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CombatOver => "combat is over",
            Self::DecisionPending => "a decision is pending",
            Self::NoDecisionPending => "no decision is pending",
            Self::InvalidDecision => "the decision does not answer the request",
            Self::CardNotInHand => "the card is not in hand",
            Self::Unplayable => "the card cannot be played",
            Self::NotEnoughEnergy => "not enough energy",
            Self::NotEnoughStars => "not enough stars",
            Self::InvalidTarget => "invalid target",
            Self::NoPotionInSlot => "no potion in that slot",
            Self::PotionNotUsable => "the potion cannot be used now",
        })
    }
}

impl std::error::Error for ActionError {}

/// The unit of replay: one top-level step of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Step {
    StartCombat,
    PlayCard {
        card: CardId,
        target: Option<CreatureId>,
    },
    UsePotion {
        slot: usize,
        target: Option<CreatureId>,
    },
    DiscardPotion {
        slot: usize,
    },
    EndTurn,
}

/// An interrupted step waiting for a decision. The state before the step is
/// [`Combat::snapshot`].
#[derive(Debug, PartialEq, Eq)]
struct Pending {
    request: DecisionRequest,
    step: Step,
    /// Whole answers given so far within this step ([`Decision::Cards`]).
    decisions: Vec<Decision>,
    /// The event count before the step started.
    events_len: usize,
}

impl_clone_by_fields!(Pending {
    request,
    step,
    decisions,
    events_len,
});

/// Whether every completed step is re-run from its snapshot and checked against the direct
/// result (on in debug builds and tests). It also makes every step take a snapshot.
const REPLAY_CHECK: bool = cfg!(debug_assertions);

/// A combat in progress.
pub struct Combat {
    state: CombatState,
    /// `None` when events are not recorded.
    events: Option<Vec<Event>>,
    pending: Option<Box<Pending>>,
    /// A retained [`Pending`] allocation for the next decision, with its lists' buffers, so
    /// that neither a step nor a copy allocates one. Not part of the combat: left out of copies,
    /// equality and `Debug`.
    spare: Option<Box<Pending>>,
    /// The state before the last snapshotted step: allocated by the first step that needs one,
    /// then refilled with `clone_from` and read back with `clone_from`, so a step allocates
    /// nothing for it. Part of the combat only while a decision is pending (the state to
    /// replay the step from); otherwise stale, and left out of copies, equality and `Debug`.
    snapshot: Option<Box<CombatState>>,
    /// Where the combat stopped, if it met unported content.
    unsupported: Option<Unsupported>,
}

impl Combat {
    /// The state to replay the pending step from; `None` without a pending decision.
    fn pending_snapshot(&self) -> Option<&CombatState> {
        self.pending.as_ref()?;
        Some(
            self.snapshot
                .as_deref()
                .expect("a pending step has a snapshot"),
        )
    }
}

/// Copies `from` into the retained snapshot, allocating it the first time.
fn refill(snapshot: &mut Option<Box<CombatState>>, from: &CombatState) {
    match snapshot {
        Some(to) => (**to).clone_from(from),
        None => *snapshot = Some(Box::new(from.clone())),
    }
}

impl Clone for Combat {
    fn clone(&self) -> Self {
        let Self {
            state,
            events,
            pending,
            spare: _,
            snapshot: _,
            unsupported,
        } = self;
        Self {
            state: state.clone(),
            events: events.clone(),
            pending: pending.clone(),
            spare: None,
            snapshot: self.pending_snapshot().map(|s| Box::new(s.clone())),
            unsupported: *unsupported,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let Self {
            state,
            events,
            pending,
            spare: _,
            snapshot: _,
            unsupported,
        } = source;
        self.state.clone_from(state);
        self.events.clone_from(events);
        // Not `Option::clone_from`, which drops the old box (or allocates a new one) whenever
        // only one side is pending.
        match (&mut self.pending, pending) {
            (Some(to), Some(from)) => (**to).clone_from(from),
            (None, Some(from)) => {
                self.pending = Some(self.spare.take().map_or_else(
                    || from.clone(),
                    |mut to| {
                        (*to).clone_from(from);
                        to
                    },
                ));
            }
            (Some(_), None) => self.spare = self.pending.take(),
            (None, None) => {}
        }
        if let Some(from) = source.pending_snapshot() {
            refill(&mut self.snapshot, from);
        }
        self.unsupported = *unsupported;
        debug_assert!(self == source, "`Combat::clone_from` made an unequal copy");
    }
}

impl PartialEq for Combat {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
            && self.events == other.events
            && self.pending == other.pending
            && self.pending_snapshot() == other.pending_snapshot()
            && self.unsupported == other.unsupported
    }
}

impl Eq for Combat {}

impl fmt::Debug for Combat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Combat")
            .field("state", &self.state)
            .field("events", &self.events)
            .field("pending", &self.pending)
            .field("snapshot", &self.pending_snapshot())
            .field("unsupported", &self.unsupported)
            .finish_non_exhaustive()
    }
}

impl Combat {
    /// Sets up the combat (`SetUpCombat`) and runs it to the first decision point.
    ///
    /// # Panics
    /// If `config` has more potions than potion slots.
    #[must_use]
    pub fn new(config: CombatConfig) -> Self {
        let events = config.record_events.then(Vec::new);
        let mut combat = Self {
            state: set_up(config),
            events,
            pending: None,
            spare: None,
            snapshot: None,
            unsupported: None,
        };
        combat.run(Step::StartCombat, Vec::new(), false);
        combat
    }

    /// The current state. While a decision is pending this is the state at the choice.
    #[must_use]
    pub const fn state(&self) -> &CombatState {
        &self.state
    }

    /// The state as [`Hooked`]: the hook-aware queries ([`Hooked::energy_cost`],
    /// [`Hooked::keywords`], [`Hooked::hittable_enemies`], ...).
    #[must_use]
    pub const fn hooked(&self) -> Hooked<'_> {
        self.state.hooked()
    }

    /// `CardEnergyCost.GetWithModifiers(All)` of a combat card ([`Hooked::energy_cost`]).
    #[must_use]
    pub fn energy_cost(&self, card: CardId) -> i32 {
        self.hooked().energy_cost(card)
    }

    /// Everything that happened so far; empty unless [`CombatConfig::record_events`].
    #[must_use]
    pub fn events(&self) -> &[Event] {
        self.events.as_deref().unwrap_or_default()
    }

    /// The pending decision, if any.
    #[must_use]
    pub fn pending_decision(&self) -> Option<&DecisionRequest> {
        self.pending.as_ref().map(|p| &p.request)
    }

    /// Where the combat stands.
    #[must_use]
    pub const fn status(&self) -> Status {
        if let Some(unsupported) = self.unsupported {
            return Status::Unsupported(unsupported);
        }
        match self.state.result {
            Some(CombatResult::Won) => Status::Won,
            Some(CombatResult::Lost) => Status::Lost,
            None if self.pending.is_some() => Status::Decision,
            None => Status::PlayerTurn,
        }
    }

    /// Ends the combat's ownership of the run's RNG streams and returns them.
    #[must_use]
    pub fn into_rng(self) -> RunRngSet {
        self.state.rng
    }

    /// Every action [`Combat::apply`] accepts now. A card choice is offered one pick at a
    /// time ([`Decision::Pick`], then [`Decision::Done`]).
    #[must_use]
    pub fn legal_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        self.legal_actions_into(&mut actions);
        actions
    }

    /// [`Combat::legal_actions`] into `actions` (cleared first), reusing its allocation.
    pub fn legal_actions_into(&self, actions: &mut Vec<Action>) {
        actions.clear();
        if self.status() == Status::PlayerTurn {
            let p = &self.state.player;
            actions.reserve(p.hand.len() * self.state.enemies.len().max(1) + p.potions.len() + 1);
        }
        self.for_each_legal_action(|action| actions.push(action));
    }

    /// Calls `f` with each action of [`Combat::legal_actions`], in order, without a list.
    pub fn for_each_legal_action(&self, mut f: impl FnMut(Action)) {
        match self.status() {
            Status::Won | Status::Lost | Status::Unsupported(_) => {}
            Status::Decision => {
                if let Some(request) = self.pending_decision() {
                    request
                        .options()
                        .into_iter()
                        .map(Action::Resolve)
                        .for_each(f);
                }
            }
            Status::PlayerTurn => {
                let s = &self.state;
                // Nothing changes between the candidates, so they share one `Hooked`, and
                // `CanPlay` does not depend on the target: [`Combat::check_play_with`] per card.
                {
                    let h = self.hooked();
                    for &card in &self.state.player.hand {
                        if can_play_with(h, card).is_err() {
                            continue;
                        }
                        let target_type = s.target_type(card);
                        for target in Self::card_targets(s, target_type) {
                            if Self::is_valid_target(s, target_type, target) {
                                f(Action::PlayCard { card, target });
                            }
                        }
                    }
                };
                for slot in 0..self.state.player.potions.len() {
                    for target in self.potion_targets(slot) {
                        if self.check_use_potion(slot, target).is_ok() {
                            f(Action::UsePotion { slot, target });
                        }
                    }
                }
                f(Action::EndTurn);
            }
        }
    }

    /// Applies a player action.
    ///
    /// # Errors
    /// If the action is not legal now; the combat is then unchanged.
    pub fn apply(&mut self, action: Action) -> Result<Status, ActionError> {
        match self.status() {
            Status::Won | Status::Lost | Status::Unsupported(_) => {
                return Err(ActionError::CombatOver);
            }
            Status::Decision => {
                let Action::Resolve(decision) = action else {
                    return Err(ActionError::DecisionPending);
                };
                let Some(pending) = self.pending.as_mut() else {
                    unreachable!("`Status::Decision` has a pending request");
                };
                if !pending.request.accepts(&decision) {
                    return Err(ActionError::InvalidDecision);
                }
                let cards = match decision {
                    Decision::Pick(card) => {
                        pending.request.pick(card);
                        if !pending.request.is_complete() {
                            return Ok(Status::Decision);
                        }
                        pending.request.take_picked()
                    }
                    Decision::Unpick(card) => {
                        pending.request.unpick(card);
                        return Ok(Status::Decision);
                    }
                    Decision::Done => pending.request.take_picked(),
                    Decision::Cards(cards) => cards,
                };
                let Some(mut pending) = self.pending.take() else {
                    unreachable!("`Status::Decision` has a pending request");
                };
                let step = pending.step;
                let events_len = pending.events_len;
                let mut decisions = std::mem::take(&mut pending.decisions);
                self.spare = Some(pending);
                decisions.push(Decision::Cards(cards));
                let Some(snapshot) = self.snapshot.as_deref() else {
                    unreachable!("a pending step has a snapshot");
                };
                self.state.clone_from(snapshot);
                if let Some(events) = &mut self.events {
                    events.truncate(events_len);
                }
                // The snapshot already holds the state the step restarts from.
                self.run(step, decisions, true);
            }
            Status::PlayerTurn => match action {
                Action::Resolve(_) => return Err(ActionError::NoDecisionPending),
                Action::PlayCard { card, target } => {
                    self.check_play(card, target)?;
                    self.run(Step::PlayCard { card, target }, Vec::new(), false);
                }
                Action::UsePotion { slot, target } => {
                    // EnqueueManualUse: no target means the owner, if that is valid.
                    let target = target.or_else(|| {
                        let me = self.state.player.creature;
                        self.check_use_potion(slot, Some(me)).is_ok().then_some(me)
                    });
                    self.check_use_potion(slot, target)?;
                    self.run(Step::UsePotion { slot, target }, Vec::new(), false);
                }
                Action::EndTurn => self.run(Step::EndTurn, Vec::new(), false),
            },
        }
        Ok(self.status())
    }

    /// The belt slots whose potion [`Combat::discard_potion`] accepts now. Not part of
    /// [`Combat::legal_actions`]: discarding is a UI affordance, not a move a policy should pick.
    #[must_use]
    pub fn discardable_potions(&self) -> Vec<usize> {
        if self.status() != Status::PlayerTurn {
            return Vec::new();
        }
        self.state
            .player
            .potions
            .iter()
            .enumerate()
            .filter_map(|(slot, p)| p.map(|_| slot))
            .collect()
    }

    /// `PotionCmd.Discard` on the potion in belt slot `slot`: the slot empties and
    /// `AfterPotionDiscarded` runs.
    ///
    /// # Errors
    /// If it is not the play phase or the slot is empty; the combat is then unchanged.
    pub fn discard_potion(&mut self, slot: usize) -> Result<Status, ActionError> {
        if self.status() != Status::PlayerTurn {
            return Err(match self.status() {
                Status::Decision => ActionError::DecisionPending,
                _ => ActionError::CombatOver,
            });
        }
        if self
            .state
            .player
            .potions
            .get(slot)
            .copied()
            .flatten()
            .is_none()
        {
            return Err(ActionError::NoPotionInSlot);
        }
        self.run(Step::DiscardPotion { slot }, Vec::new(), false);
        Ok(self.status())
    }

    /// Runs `step` from the current state with the given decision log; on an interrupt,
    /// records the pending decision, with the pre-step state in [`Combat::snapshot`]. The
    /// snapshot is only taken while some model in combat may prompt
    /// ([`CombatState::may_prompt`]), and not again when `snapshotted` (it already holds the
    /// current state). Unported content stops the combat where it was met
    /// ([`Status::Unsupported`]).
    ///
    /// # Panics
    /// If the step prompts without a snapshot (a model's `may_prompt` is wrong), or, with
    /// [`REPLAY_CHECK`], if re-running the step from its snapshot gives a different result.
    fn run(&mut self, step: Step, decisions: Vec<Decision>, snapshotted: bool) {
        // Checked even when the snapshot is taken anyway, so that debug builds (which always
        // snapshot for the replay check) still catch a missing `may_prompt`.
        let may_prompt = self.state.may_prompt();
        let snapshot = REPLAY_CHECK || may_prompt;
        if snapshot && !snapshotted {
            refill(&mut self.snapshot, &self.state);
        }
        let events_len = self.events.as_ref().map_or(0, Vec::len);
        match execute(&mut self.state, self.events.as_mut(), &step, &decisions)
            .map_err(|interrupt| *interrupt)
        {
            Ok(()) => {
                if REPLAY_CHECK {
                    self.check_replay(events_len, &step, &decisions);
                }
                self.pending = None;
            }
            Err(Interrupt::Decision(request)) => {
                assert!(
                    may_prompt,
                    "{step:?} prompted, but no model in combat reports `may_prompt`"
                );
                let pending = Pending {
                    request,
                    step,
                    decisions,
                    events_len,
                };
                self.pending = Some(match self.spare.take() {
                    Some(mut to) => {
                        *to = pending;
                        to
                    }
                    None => Box::new(pending),
                });
            }
            Err(Interrupt::Unsupported(unsupported)) => {
                self.pending = None;
                self.unsupported = Some(unsupported);
            }
            Err(Interrupt::Hidden) => unreachable!("only previews stop at a draw"),
        }
    }

    /// Re-runs a completed step from [`Combat::snapshot`] (`events_len` events before it) and
    /// asserts that the state and events equal the direct result.
    fn check_replay(&self, events_len: usize, step: &Step, decisions: &[Decision]) {
        let mut state = self
            .snapshot
            .as_deref()
            .expect("every step snapshots with `REPLAY_CHECK`")
            .clone();
        let mut events = self.events.as_ref().map(|_| Vec::new());
        let result = execute(&mut state, events.as_mut(), step, decisions);
        assert_eq!(result, Ok(()), "replaying {step:?} prompted");
        assert!(state == self.state, "replaying {step:?} changed the state");
        if let (Some(direct), Some(replayed)) = (&self.events, &events) {
            assert_eq!(
                &direct[events_len..],
                replayed.as_slice(),
                "replaying {step:?} changed the events"
            );
        }
    }

    /// Candidate targets for a card of `target_type`.
    fn card_targets(
        s: &CombatState,
        target_type: TargetType,
    ) -> impl Iterator<Item = Option<CreatureId>> + '_ {
        let (untargeted, enemies): (bool, &[CreatureId]) = match target_type {
            TargetType::Myself
            | TargetType::None
            | TargetType::AllEnemies
            | TargetType::RandomEnemy => (true, &[]),
            TargetType::AnyEnemy => (false, &s.enemies),
            // No in-scope card targets a player.
            TargetType::AnyPlayer => (false, &[]),
        };
        untargeted
            .then_some(None)
            .into_iter()
            .chain(enemies.iter().map(|&e| Some(e)))
    }

    /// Candidate targets for the potion in `slot` (none for an empty slot): every target
    /// [`Combat::check_use_potion`] can accept. Unlike a card, a `Self` potion names its owner
    /// (`PotionModel.IsValidTarget`'s NOTE). Inlined into [`Combat::for_each_legal_action`],
    /// where LLVM drops the list's allocation.
    #[expect(
        clippy::inline_always,
        reason = "measured: out of line (RC1's `for_each_legal_action`), each potion slot allocates its list, +104 instructions per W1 action"
    )]
    #[inline(always)]
    fn potion_targets(&self, slot: usize) -> Vec<Option<CreatureId>> {
        let s = &self.state;
        let Some(potion) = s.player.potions.get(slot).copied().flatten() else {
            return Vec::new();
        };
        match potion.kind.target() {
            TargetType::None | TargetType::AllEnemies | TargetType::RandomEnemy => vec![None],
            TargetType::Myself => vec![Some(s.player.creature)],
            TargetType::AnyEnemy => s.enemies.iter().map(|&e| Some(e)).collect(),
            TargetType::AnyPlayer => s.player_creatures().into_iter().map(Some).collect(),
        }
    }

    /// The use button's checks (`NPotionPopup`: owner alive, `Usage`) and
    /// `PotionModel.IsValidTarget` (`PotionModel.cs:260`), for a target in the combat (the
    /// game's UI offers no other). Only called in the play phase.
    fn check_use_potion(&self, slot: usize, target: Option<CreatureId>) -> Result<(), ActionError> {
        let s = &self.state;
        let Some(potion) = s.player.potions.get(slot).copied().flatten() else {
            return Err(ActionError::NoPotionInSlot);
        };
        let usable = match potion.kind.usage() {
            PotionUsage::CombatOnly | PotionUsage::AnyTime => true,
            PotionUsage::Automatic => false,
        };
        if !usable || s.creature(s.player.creature).is_dead() {
            return Err(ActionError::PotionNotUsable);
        }
        let owner = s.creature(s.player.creature);
        // `!TargetType.IsSingleTarget()`: of the in-scope target types, `None`, `AllEnemies`
        // and `RandomEnemy`.
        let valid_target = target.map_or_else(
            || {
                matches!(
                    potion.kind.target(),
                    TargetType::None | TargetType::AllEnemies | TargetType::RandomEnemy
                )
            },
            |t| {
                if !s.contains_creature(t) {
                    return false;
                }
                let c = s.creature(t);
                c.is_alive()
                    && match potion.kind.target() {
                        TargetType::AnyEnemy => c.side != owner.side,
                        TargetType::AnyPlayer => c.is_player(),
                        TargetType::Myself => t == owner.id,
                        TargetType::None | TargetType::AllEnemies | TargetType::RandomEnemy => {
                            false
                        }
                    }
            },
        );
        if valid_target {
            Ok(())
        } else {
            Err(ActionError::InvalidTarget)
        }
    }

    /// `CanPlay` and `IsValidTarget` for a manual play.
    fn check_play(&self, card: CardId, target: Option<CreatureId>) -> Result<(), ActionError> {
        Self::check_play_with(self.hooked(), card, target)
    }

    /// Changes the state as no step would, then brings what is derived from it back in step:
    /// the hook unions and the counts. Tests run the same step code as every other build.
    /// Unit tests set up positions this way, and other crates' tests (the `testing`
    /// feature) change what the player cannot see, such as the draw pile's order or the RNG
    /// streams, to check that an observation does not depend on it.
    #[cfg(any(test, feature = "testing"))]
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut CombatState) -> R) -> R {
        let result = f(&mut self.state);
        self.resync();
        result
    }

    /// A copy of this combat with what the player cannot see resampled: the draw and discard
    /// pile order and the RNG streams (the run's and each monster's), seeded from `seed`. The
    /// hand, HP, powers, intents, relics and pile contents stay as they are. For honest search:
    /// every determinization starts from the same visible position and a plausible hidden one.
    /// The piles are put in card order before they are shuffled, so the world depends on the
    /// real order not at all (a shuffle of the real order would be a fixed relabelling of it).
    #[must_use]
    pub fn blindfold_for_search(&self, seed: u64) -> Self {
        let mut combat = self.clone();
        let mut shuffle = Rng::new(seed);
        combat.state.player.draw.sort_unstable();
        combat.state.player.discard.sort_unstable();
        unstable_shuffle(&mut combat.state.player.draw[..], &mut shuffle);
        unstable_shuffle(&mut combat.state.player.discard[..], &mut shuffle);
        combat.state.rng = RunRngSet::new(&format!("SEARCH{seed}"));
        for (i, creature) in combat.state.creatures.iter_mut().enumerate() {
            if let Some(monster) = &mut creature.monster {
                let index = u64::try_from(i).unwrap_or(u64::MAX);
                monster.rng = Rng::new(seed ^ 0x5EED_0000 ^ index);
            }
        }
        combat
    }

    /// The part of [`Combat::edit`] after the change.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn resync(&mut self) {
        self.state.recount_hooks();
        self.state.recount_enemies();
        self.state.recount_prompting();
    }

    /// [`Combat::check_play`] on [`Hooked`].
    fn check_play_with(
        h: Hooked<'_>,
        card: CardId,
        target: Option<CreatureId>,
    ) -> Result<(), ActionError> {
        let s = h.s;
        if !s.player.hand.contains(&card) {
            return Err(ActionError::CardNotInHand);
        }
        can_play_with(h, card)?;
        if Self::is_valid_target(s, s.target_type(card), target) {
            Ok(())
        } else {
            Err(ActionError::InvalidTarget)
        }
    }

    /// `IsValidTarget` (`CardModel.cs:1762`) for a card of `target_type`.
    fn is_valid_target(
        s: &CombatState,
        target_type: TargetType,
        target: Option<CreatureId>,
    ) -> bool {
        match (target_type, target) {
            (
                TargetType::Myself
                | TargetType::None
                | TargetType::AllEnemies
                | TargetType::RandomEnemy,
                None,
            ) => true,
            (TargetType::AnyEnemy, Some(t)) => s.enemies.contains(&t) && s.creature(t).is_alive(),
            _ => false,
        }
    }
}

/// `CanPlay` (`CardModel.cs:1725-1754`): the Unplayable keyword, enough energy and stars
/// (`HasEnoughResourcesFor`, `PlayerCombatState.cs:190-209`; an X cost needs 0,
/// `ShouldPayExcessEnergyCostWithStars` has no listener), `Hook.ShouldPlay`, `IsPlayable`.
/// (`NoLivingAllies` is for `AnyAlly`, a multiplayer target.)
pub(crate) fn can_play(s: &CombatState, card: CardId) -> Result<(), ActionError> {
    can_play_with(s.hooked(), card)
}

/// [`can_play`] on [`Hooked`]. Usually nothing can change a card's keywords or costs or
/// veto its play, and the card has no star cost: then its own fields decide, inline (callers
/// such as [`Combat::legal_actions_into`] ask this of every card in hand); else
/// [`can_play_hooked`]. Debug builds check the inline answer against it.
#[inline]
pub(crate) fn can_play_with(h: Hooked<'_>, card: CardId) -> Result<(), ActionError> {
    /// The hooks that can change what [`can_play_with`] answers beyond the card's own fields.
    const HOOKS: HookSet = HookSet::EMPTY
        .with(Hook::try_modify_keywords_in_combat)
        .with(Hook::try_modify_energy_cost_in_combat)
        .with(Hook::try_modify_energy_cost_in_combat_late)
        .with(Hook::should_play);
    let s = h.s;
    let c = s.card(card);
    if s.hooks().intersects(HOOKS)
        || !s.cost_lists.is_empty()
        || c.star_cost.costs_x
        || c.star_cost.base >= 0
    {
        return can_play_hooked(h, card);
    }
    let result = if c.keywords.contains(CardKeyword::Unplayable) {
        Err(ActionError::Unplayable)
    } else if s.player.energy < c.energy_cost.base.max(0) {
        Err(ActionError::NotEnoughEnergy)
    } else if !crate::content::cards::is_playable(s, card) {
        Err(ActionError::Unplayable)
    } else {
        Ok(())
    };
    debug_assert_eq!(
        result,
        can_play_hooked(h, card),
        "the inline `CanPlay` of {card:?}"
    );
    result
}

/// [`can_play_with`], the general case.
#[inline(never)]
fn can_play_hooked(h: Hooked<'_>, card: CardId) -> Result<(), ActionError> {
    let s = h.s;
    if h.has_keyword(card, CardKeyword::Unplayable) {
        return Err(ActionError::Unplayable);
    }
    // Without local modifiers or cost hooks the energy cost is the base, without the call
    // ([`Hooked::energy_cost`]); a card without a star cost needs 0 stars ([`Hooked::star_cost`]).
    let c = s.card(card);
    let energy = if s.cost_lists.energy(card).is_empty()
        && !h.has(Hook::try_modify_energy_cost_in_combat)
        && !h.has(Hook::try_modify_energy_cost_in_combat_late)
    {
        c.energy_cost.base.max(0)
    } else {
        h.energy_cost(card).max(0)
    };
    if s.player.energy < energy {
        return Err(ActionError::NotEnoughEnergy);
    }
    let star_cost = &c.star_cost;
    let stars = if !star_cost.costs_x && star_cost.base < 0 && s.cost_lists.star(card).is_empty() {
        0
    } else {
        h.star_cost(card).max(0)
    };
    if s.player.stars < stars {
        return Err(ActionError::NotEnoughStars);
    }
    if !h.should_play(card, AutoPlayType::None) || !crate::content::cards::is_playable(s, card) {
        return Err(ActionError::Unplayable);
    }
    Ok(())
}

/// Runs `step` on `state` with the decision log.
fn execute(
    state: &mut CombatState,
    events: Option<&mut Vec<Event>>,
    step: &Step,
    decisions: &[Decision],
) -> Flow {
    execute_on(&mut Engine::new(state, events, decisions), step)
}

/// Runs `step` on `eng`.
fn execute_on(eng: &mut Engine<'_>, step: &Step) -> Flow {
    match *step {
        Step::StartCombat => eng.start_combat(),
        Step::PlayCard { card, target } => {
            eng.play_card_action(card, target)?;
            eng.end_turn_if_ready()
        }
        Step::UsePotion { slot, target } => {
            eng.use_potion_action(slot, target)?;
            eng.end_turn_if_ready()
        }
        Step::DiscardPotion { slot } => eng.discard_potion(slot),
        Step::EndTurn => eng.end_player_turn(),
    }
}

/// `CombatRoom.StartCombat` and `SetUpCombat` (`CombatRoom.cs:194-227`,
/// `CombatManager.cs:444`): generate the encounter's monsters and create them (HP rolled on
/// `Rng.Niche`, `Rng` seeded per creature), then the player's draw pile (deck order, then
/// `UnstableShuffle` on `Rng.Shuffle`). The player creature was attached first (`CombatId` 0).
#[expect(
    clippy::too_many_lines,
    reason = "one struct literal for a fresh combat"
)]
fn set_up(config: CombatConfig) -> CombatState {
    let CombatConfig {
        character,
        rng,
        ascension,
        encounter,
        room_type,
        deck,
        relics,
        potion_slots,
        potions,
        player_hp,
        player_max_hp,
        max_energy,
        gold,
        total_floor,
        act_index,
        map_coord,
        record_events: _,
        modifiers,
    } = config;
    assert!(
        potions.len() <= potion_slots,
        "{} potions do not fit in {potion_slots} slots",
        potions.len()
    );
    let (potions, unported_potions) = (
        belt(&potions, potion_slots),
        unported_belt(&potions, potion_slots),
    );
    let player_id = CreatureId(0);
    let cards = combat_cards(&deck);
    let draw = cards.iter().map(|c| c.id).collect();
    let monsters = encounter.generate_monsters(&rng, total_floor);
    let mut state = CombatState {
        rng,
        ascension,
        encounter,
        deck,
        room_type,
        act_index,
        map_coord,
        round: 1,
        side: Side::Player,
        creatures: vec![Creature {
            id: player_id,
            side: Side::Player,
            hp: player_hp,
            max_hp: player_max_hp,
            block: 0,
            powers: Vec::new(),
            removed_powers: Vec::new(),
            power_kinds: PowerKinds::default(),
            hooks: HookSet::EMPTY,
            monster_hooks: HookSet::EMPTY,
            monster: None,
            pet_owner: None,
            slot: None,
            infinite_hp_display: false,
        }],
        allies: vec![player_id],
        enemies: Vec::new(),
        player: Player {
            character,
            creature: player_id,
            energy: 0,
            max_energy,
            gold,
            turn_number: 1,
            extra_turn: false,
            phase: TurnPhase::None,
            relics: relics
                .into_iter()
                .zip(0..)
                .map(|(relic, i)| Relic {
                    id: RelicId(i),
                    kind: relic.kind,
                    data: relic.data,
                    melted: relic.melted,
                })
                .collect(),
            potions,
            unported_potions,
            draw,
            hand: state::Pile::default(),
            discard: state::Pile::default(),
            exhaust: state::Pile::default(),
            play: state::Pile::default(),
            pets: Vec::new(),
            orb_slots: character.base_orb_slots(),
            orbs: Vec::new(),
            stars: 0,
            ready_to_end_turn: false,
            effect_depth: 0,
            hooks_active: true,
            item_hooks: HookSet::EMPTY,
        },
        cards,
        cost_lists: CostLists::default(),
        power_data: state::PowerDataTable::default(),
        in_progress: false,
        modifiers,
        starting: true,
        pending_loss: false,
        result: None,
        next_power_id: 0,
        next_potion_id: u32::try_from(potion_slots).expect("potion slot count fits u32"),
        next_attack_id: 0,
        next_orb_id: 0,
        extra_rewards: Vec::new(),
        removed_from_deck: Vec::new(),
        escaped: Vec::new(),
        spawned_enemies: Vec::new(),
        encounter_data: EncounterData::None,
        gold_proportion: GoldProportion::ONE,
        history: CombatHistory::default(),
        living_primary_enemies: 0,
        prompting: state::PromptCount::default(),
        hook_index: listeners::HookIndex::default(),
    };
    for (kind, slot) in monsters {
        let id = create_enemy(&mut state, kind, slot);
        // AddCreature, then CombatManager.AddCreature (`CombatManager.cs:1096-1110`).
        state.push_enemy(id);
        if slot.is_some() {
            state.sort_enemies_by_slot_name();
        }
    }
    // PopulateCombatState -> RandomizeOrderInternal. Hook.ModifyShuffleOrder with
    // isInitialShuffle: its one listener (`PerfectFit`) ignores the initial shuffle.
    unstable_shuffle(&mut state.player.draw, &mut state.rng[RunRngType::Shuffle]);
    state.recount_prompting();
    state.recount_hooks();
    state
}

/// The combat clones of the deck cards, in the draw pile (ids `0..` in deck order).
fn combat_cards(deck: &[DeckCard]) -> Vec<Card> {
    deck.iter()
        .zip(0..)
        .map(|(c, i)| {
            let mut card = Card::from_deck(CardId(i), *c);
            card.deck_slot = Some(i);
            card.pile = Some(PileType::Draw);
            card
        })
        .collect()
}

/// The potion belt's ported potions by slot (each potion's id is its slot); `slots` long.
fn belt(potions: &[Option<BeltPotion>], slots: usize) -> Vec<Option<Potion>> {
    let mut belt: Vec<Option<Potion>> = potions
        .iter()
        .zip(0..)
        .map(|(p, i)| match p {
            Some(BeltPotion::Ported(kind)) => Some(Potion {
                id: PotionId(i),
                kind: *kind,
            }),
            _ => None,
        })
        .collect();
    belt.resize(slots, None);
    belt
}

/// The slots of the belt's unported potions; `slots` long.
fn unported_belt(potions: &[Option<BeltPotion>], slots: usize) -> Vec<Option<&'static str>> {
    let mut belt: Vec<Option<&'static str>> = potions
        .iter()
        .map(|p| match p {
            Some(BeltPotion::Unported(id)) => Some(*id),
            _ => None,
        })
        .collect();
    belt.resize(slots, None);
    belt
}

/// `CombatState.CreateCreature` for an enemy (`CombatState.cs:233-248`): the unique HP roll on
/// `Rng.Niche` against the enemies in combat, the next `CombatId`, the monster's `Rng` and
/// `SetUpForCombat`. The creature is not in `Enemies` yet.
pub(crate) fn create_enemy(
    state: &mut CombatState,
    kind: MonsterKind,
    slot: Option<&'static str>,
) -> CreatureId {
    let hp = roll_unique_monster_hp(state, kind);
    let id = CreatureId(u32::try_from(state.creatures.len()).expect("creature count fits u32"));
    let rng = monster_rng(state, id);
    // `Encounter.OnCreatureSpawned`: each monster model counts once.
    if !state.spawned_enemies.contains(&kind.id()) {
        state.spawned_enemies.push(kind.id());
    }
    state.push_creature(Creature {
        id,
        side: Side::Enemy,
        hp,
        max_hp: hp,
        block: 0,
        powers: Vec::new(),
        removed_powers: Vec::new(),
        power_kinds: PowerKinds::default(),
        hooks: HookSet::EMPTY,
        monster_hooks: HookSet::EMPTY,
        monster: Some(Monster::new(kind, rng)),
        pet_owner: None,
        slot,
        infinite_hp_display: false,
    });
    id
}

/// `MonsterModel.Rng` as seeded in `CombatState.CreateCreature` (`CombatState.cs:245`):
/// `run seed + col + row + CurrentActIndex + CombatId`.
pub(crate) fn monster_rng(state: &CombatState, id: CreatureId) -> Rng {
    let (col, row) = state.map_coord.unwrap_or((0, 0));
    let seed = state
        .rng
        .seed()
        .cast_signed()
        .wrapping_add(i64::from(col))
        .wrapping_add(i64::from(row))
        .wrapping_add(i64::from(state.act_index))
        .wrapping_add(i64::from(id.0));
    Rng::new(seed.cast_unsigned())
}

/// `Creature.SetUniqueMonsterHpValue` (`Creature.cs:372-384`): pick from `min..=max` minus the
/// max HP of the other creatures on the side, on `Rng.Niche`; if nothing is left,
/// `NextInt(min, max + 1)`.
fn roll_unique_monster_hp(state: &mut CombatState, kind: MonsterKind) -> i32 {
    let min = kind.min_initial_hp(state.ascension);
    let max = kind.max_initial_hp(state.ascension);
    let taken: Vec<i32> = state
        .enemies
        .iter()
        .map(|&e| state.creature(e).max_hp)
        .collect();
    let candidates: Vec<i32> = (min..=max).filter(|hp| !taken.contains(hp)).collect();
    let rng = &mut state.rng[RunRngType::Niche];
    if candidates.is_empty() {
        rng.next_int_range(min, max + 1)
    } else {
        *rng.next_item(&candidates).expect("candidates is non-empty")
    }
}

/// Test access for the run's tests: runs `f` on an engine over the state, as a model would.
#[cfg(test)]
impl Combat {
    pub(crate) fn with_engine<T>(&mut self, f: impl FnOnce(&mut Engine<'_>) -> Flow<T>) -> T {
        let mut eng = Engine::new(&mut self.state, self.events.as_mut(), &[]);
        let result = f(&mut eng).expect("no decision needed");
        self.resync();
        result
    }
}

#[cfg(test)]
mod tests;
