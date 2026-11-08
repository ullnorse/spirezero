//! The execution context every effect runs in (execution model C).
//!
//! Effects are ordinary nested calls that mirror the game's `await` chains. A call that needs a
//! player choice either takes the next answer from the decision log or aborts the whole action
//! with [`Interrupt::Decision`]; the caller then restores the pre-action snapshot and replays
//! the action once the answer is known.

use std::fmt;

use super::hooks::{Hook, Hooked, PowerRef};
use super::state::{CardId, CombatState, CreatureId, PileType, RelicId, Side};
use crate::content::potions::PotionKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// Why an action stopped before finishing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interrupt {
    /// A player choice with no recorded answer.
    Decision(DecisionRequest),
    /// The effect needs content the engine does not port; the combat stops there.
    Unsupported(Unsupported),
    /// A preview ([`Engine::stop_at_hidden`]) reached a draw or a shuffle, whose cards the
    /// player cannot see. Never returned to a real step.
    Hidden,
}

/// What an [`Unsupported`] stop names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContentKind {
    /// A card model.
    Card,
    /// A potion model.
    Potion,
    /// A relic model.
    Relic,
}

impl ContentKind {
    /// The kind in lower case (`card`), as stop reasons print it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Potion => "potion",
            Self::Relic => "relic",
        }
    }
}

/// An effect met content that is not ported (or is out of scope, `docs/coverage-exclusions.md`),
/// such as a generated card. The RNG has advanced exactly as the game's would up to that point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unsupported {
    /// What kind of content it is.
    pub kind: ContentKind,
    /// `ModelId.Entry`.
    pub id: &'static str,
}

impl Unsupported {
    /// An unported card.
    #[must_use]
    pub const fn card(id: &'static str) -> Self {
        Self {
            kind: ContentKind::Card,
            id,
        }
    }

    /// An unported potion.
    #[must_use]
    pub const fn potion(id: &'static str) -> Self {
        Self {
            kind: ContentKind::Potion,
            id,
        }
    }

    /// An unported relic effect.
    #[must_use]
    pub const fn relic(id: &'static str) -> Self {
        Self {
            kind: ContentKind::Relic,
            id,
        }
    }

    /// The stop as an [`Interrupt`], to return from an effect.
    pub(crate) fn stop<T>(self) -> Flow<T> {
        Err(Box::new(Interrupt::Unsupported(self)))
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind.name(), self.id)
    }
}

/// The result of every effect step: `Err` unwinds the whole action.
///
/// The [`Interrupt`] is boxed: it is rare, and a `Flow<()>` then fits a register, so each `?`
/// on the hot path tests a pointer instead of copying a large result.
pub type Flow<T = ()> = Result<T, Box<Interrupt>>;

/// What asked for a [`DecisionRequest`]: the model whose effect is choosing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptSource {
    /// A card being played or reacting to a hook.
    Card(CardId),
    /// A potion being used.
    Potion(PotionKind),
    /// A relic of the player's.
    Relic(RelicId),
    /// A power on `owner`.
    /// A power of `kind` on `owner`.
    Power {
        /// The power's owner.
        owner: CreatureId,
        /// The power.
        kind: PowerKind,
    },
    /// A monster's move (`KnowledgeDemon`'s curse choice).
    Monster(CreatureId),
}

/// A [`DecisionRequest`]'s text (`CardSelectorPrefs.Prompt`; the choose-a-card screen's header).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptText {
    /// The source's `SelectionScreenPrompt`: `{ID}.selectionScreenPrompt` in its model's table
    /// (`cards`, `potions`, `relics`, `powers`), with the model's vars.
    Own,
    /// `CardSelectorPrefs.DiscardSelectionPrompt`.
    Discard,
    /// `CardSelectorPrefs.ExhaustSelectionPrompt`.
    Exhaust,
    /// `CardSelectorPrefs.TransformSelectionPrompt`.
    Transform,
    /// `CardSelectorPrefs.RemoveSelectionPrompt` (deck selections).
    Remove,
    /// `CardSelectorPrefs.UpgradeSelectionPrompt` (deck selections).
    Upgrade,
    /// `CardSelectorPrefs.EnchantSelectionPrompt` (deck selections).
    Enchant,
    /// `FromHandForUpgrade`'s hand prompt (`CardSelectCmd.cs:943`).
    UpgradeFromHand,
    /// `FromChooseACardScreen`'s banner (`NChooseACardSelectionScreen.cs:255`).
    ChooseACard,
}

impl PromptText {
    /// The localization table and key of a shared prompt; `None` for [`PromptText::Own`].
    #[must_use]
    pub const fn shared_key(self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            Self::Own => return None,
            Self::Discard => ("card_selection", "TO_DISCARD"),
            Self::Exhaust => ("card_selection", "TO_EXHAUST"),
            Self::Transform => ("card_selection", "TO_TRANSFORM"),
            Self::Remove => ("card_selection", "TO_REMOVE"),
            Self::Upgrade => ("card_selection", "TO_UPGRADE"),
            Self::Enchant => ("card_selection", "TO_ENCHANT"),
            Self::UpgradeFromHand => ("gameplay_ui", "CHOOSE_CARD_UPGRADE_HEADER"),
            Self::ChooseACard => ("gameplay_ui", "CHOOSE_CARD_HEADER"),
        })
    }
}

/// Who asks a [`DecisionRequest`] and what it says. The prompt's `Amount` is the request's
/// `max`, `MinCount` its `min` (`CardSelectorPrefs` constructors).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Prompt {
    /// What asks.
    pub source: PromptSource,
    /// The prompt's text.
    pub text: PromptText,
}

impl Prompt {
    /// A prompt from `source` with `text`.
    #[must_use]
    pub const fn new(source: PromptSource, text: PromptText) -> Self {
        Self { source, text }
    }

    /// A card's prompt.
    #[must_use]
    pub const fn card(card: CardId, text: PromptText) -> Self {
        Self::new(PromptSource::Card(card), text)
    }

    /// A potion's prompt.
    #[must_use]
    pub const fn potion(kind: PotionKind, text: PromptText) -> Self {
        Self::new(PromptSource::Potion(kind), text)
    }

    /// A relic's prompt.
    #[must_use]
    pub const fn relic(relic: RelicId, text: PromptText) -> Self {
        Self::new(PromptSource::Relic(relic), text)
    }

    /// A power's prompt (the power `me` in `s`).
    pub(crate) fn power(s: &CombatState, me: PowerRef, text: PromptText) -> Self {
        let kind = s.power(me.owner, me.id).kind;
        Self::new(
            PromptSource::Power {
                owner: me.owner,
                kind,
            },
            text,
        )
    }

    /// The localization table and key of the text in `state` (the state at the choice).
    #[must_use]
    pub fn key(&self, state: &CombatState) -> Option<(&'static str, String)> {
        if let Some((table, key)) = self.text.shared_key() {
            return Some((table, key.to_owned()));
        }
        let (table, id) = match self.source {
            PromptSource::Card(card) => ("cards", state.card(card).kind.entry()),
            PromptSource::Potion(kind) => ("potions", kind.id()),
            PromptSource::Relic(relic) => {
                let index = usize::try_from(relic.0).ok()?;
                ("relics", state.player.relics.get(index)?.kind.id())
            }
            PromptSource::Power { kind, .. } => ("powers", kind.id()),
            PromptSource::Monster(_) => return None,
        };
        Some((table, format!("{id}.selectionScreenPrompt")))
    }
}

/// A player choice the engine is waiting for.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum DecisionRequest {
    /// `CardSelectCmd`: pick between `min` and `max` of `options` (card identities), one card at
    /// a time. `picked` holds the cards picked so far, in order.
    ChooseCards {
        /// The cards that can be picked.
        options: Vec<CardId>,
        /// The fewest cards the selection takes.
        min: usize,
        /// The most cards the selection takes.
        max: usize,
        /// The cards picked so far, in order.
        picked: Vec<CardId>,
        /// What asks, and its text.
        prompt: Prompt,
    },
}

/// Field by field, so that a `clone_from` (a search copy of a combat waiting for a decision)
/// reuses the lists' buffers.
impl Clone for DecisionRequest {
    fn clone(&self) -> Self {
        let Self::ChooseCards {
            options,
            min,
            max,
            picked,
            prompt,
        } = self;
        Self::ChooseCards {
            options: options.clone(),
            min: *min,
            max: *max,
            picked: picked.clone(),
            prompt: *prompt,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let (
            Self::ChooseCards {
                options,
                min,
                max,
                picked,
                prompt,
            },
            Self::ChooseCards {
                options: from_options,
                min: from_min,
                max: from_max,
                picked: from_picked,
                prompt: from_prompt,
            },
        ) = (self, source);
        crate::copy_vec_from(options, from_options);
        *min = *from_min;
        *max = *from_max;
        crate::copy_vec_from(picked, from_picked);
        *prompt = *from_prompt;
    }
}

impl DecisionRequest {
    /// Whether `decision` is a valid answer now.
    #[must_use]
    pub fn accepts(&self, decision: &Decision) -> bool {
        let Self::ChooseCards {
            options,
            min,
            max,
            picked,
            ..
        } = self;
        match decision {
            Decision::Pick(card) => {
                picked.len() < *max && options.contains(card) && !picked.contains(card)
            }
            Decision::Done => picked.len() >= *min,
            Decision::Unpick(card) => picked.contains(card),
            Decision::Cards(cards) => {
                let distinct = cards
                    .iter()
                    .enumerate()
                    .all(|(i, c)| !cards[..i].contains(c));
                picked.is_empty()
                    && (*min..=*max).contains(&cards.len())
                    && distinct
                    && cards.iter().all(|c| options.contains(c))
            }
        }
    }

    /// Every valid single-step answer: a pick of each remaining option, then `Done` once
    /// enough cards are picked. Never enumerates subsets.
    #[must_use]
    pub fn options(&self) -> Vec<Decision> {
        let Self::ChooseCards {
            options,
            min,
            max,
            picked,
            ..
        } = self;
        let mut out = Vec::new();
        if picked.len() < *max {
            out.extend(
                options
                    .iter()
                    .filter(|c| !picked.contains(c))
                    .map(|&c| Decision::Pick(c)),
            );
        }
        if picked.len() >= *min {
            out.push(Decision::Done);
        }
        out
    }

    /// The answers that put a picked card back, in pick order.
    #[must_use]
    pub fn unpicks(&self) -> Vec<Decision> {
        let Self::ChooseCards { picked, .. } = self;
        picked.iter().map(|&c| Decision::Unpick(c)).collect()
    }

    /// Takes `card` out of the picks.
    pub(crate) fn unpick(&mut self, card: CardId) {
        let Self::ChooseCards { picked, .. } = self;
        picked.retain(|&c| c != card);
    }

    /// Adds `card` to the picks.
    pub(crate) fn pick(&mut self, card: CardId) {
        let Self::ChooseCards { picked, .. } = self;
        picked.push(card);
    }

    /// Takes the picks, leaving none.
    pub(crate) fn take_picked(&mut self) -> Vec<CardId> {
        let Self::ChooseCards { picked, .. } = self;
        std::mem::take(picked)
    }

    /// Whether no further pick is possible, so the choice is made: `max` picked, or no option
    /// left.
    pub(crate) const fn is_complete(&self) -> bool {
        let Self::ChooseCards {
            options,
            max,
            picked,
            ..
        } = self;
        picked.len() >= *max || picked.len() >= options.len()
    }
}

/// A player's answer to a [`DecisionRequest`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Decision {
    /// Pick one more card of a [`DecisionRequest::ChooseCards`].
    Pick(CardId),
    /// Confirm the cards picked so far.
    Done,
    /// Put a picked card back, as a second click on a selected card does in the game's hand
    /// selection (`NPlayerHand.DeselectCard`). Never in [`DecisionRequest::options`]: a bot has no use
    /// for it; [`DecisionRequest::unpicks`] lists it for a client.
    Unpick(CardId),
    /// The whole selection at once, in selection order (how the referee answers). Only valid
    /// before any [`Decision::Pick`].
    Cards(Vec<CardId>),
}

/// `ValueProp` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueProps(u8);

impl ValueProps {
    /// No flags.
    pub const NONE: Self = Self(0);
    /// `ValueProp.Unblockable`.
    pub const UNBLOCKABLE: Self = Self(1);
    /// `ValueProp.Unpowered`.
    pub const UNPOWERED: Self = Self(1 << 1);
    /// `ValueProp.Move`.
    pub const MOVE: Self = Self(1 << 2);

    /// Both sets of flags.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `HasFlag`.
    #[must_use]
    pub const fn has(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    /// `IsPoweredAttack`: `Move` set and `Unpowered` clear (`ValuePropExtensions.cs:5`).
    #[must_use]
    pub const fn is_powered_attack(self) -> bool {
        self.has(Self::MOVE) && !self.has(Self::UNPOWERED)
    }

    /// `IsPoweredCardOrMonsterMoveBlock`: the same test as [`ValueProps::is_powered_attack`]
    /// (`ValuePropExtensions.cs:14`).
    #[must_use]
    pub const fn is_powered_card_or_monster_move_block(self) -> bool {
        self.has(Self::MOVE) && !self.has(Self::UNPOWERED)
    }

    /// `IsCardOrMonsterMove`: `Move` set (`ValuePropExtensions.cs:23`).
    #[must_use]
    pub const fn is_card_or_monster_move(self) -> bool {
        self.has(Self::MOVE)
    }
}

/// Something observable that happened, for logs and tests. Not part of the game state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// A side's turn started.
    TurnStarted {
        /// Whose turn.
        side: Side,
        /// The round number.
        round: u32,
    },
    /// The discard pile was shuffled into the draw pile.
    Shuffled,
    /// A card was drawn.
    CardDrawn(CardId),
    /// A card was played.
    CardPlayed {
        /// The card.
        card: CardId,
        /// Its target, if it has one.
        target: Option<CreatureId>,
    },
    /// The hand was flushed at the end of the turn: the cards it discarded.
    HandFlushed(Vec<CardId>),
    /// A potion was used.
    PotionUsed {
        /// The potion.
        potion: PotionKind,
        /// Its target, if it has one.
        target: Option<CreatureId>,
    },
    /// A monster performs its move.
    MonsterMove {
        /// The monster.
        monster: CreatureId,
        /// The move's id (`MoveState.Id`).
        name: &'static str,
    },
    /// An `AttackCommand` started: `hits` hits of `damage` each, before modifiers.
    Attack {
        /// Who attacks.
        attacker: CreatureId,
        /// Damage per hit before modifiers.
        damage: Decimal,
        /// The number of hits.
        hits: u32,
    },
    /// One `DamageResult`.
    Damage {
        /// Who dealt it, if a creature did.
        dealer: Option<CreatureId>,
        /// Who took it.
        receiver: CreatureId,
        /// Damage that Block absorbed.
        blocked: i32,
        /// HP the receiver lost.
        hp_lost: i32,
    },
    /// A creature gained Block.
    BlockGained {
        /// The creature.
        creature: CreatureId,
        /// Block gained, after modifiers.
        amount: i32,
    },
    /// A power's amount changed.
    PowerChanged {
        /// The power's owner.
        creature: CreatureId,
        /// The power.
        kind: PowerKind,
        /// Its amount now.
        amount: i32,
    },
    /// A summon (`OstyCmd.Summon`).
    Summoned {
        /// The summon amount.
        amount: i32,
    },
    /// A creature died.
    Died(CreatureId),
    /// `CreatureCmd.Escape`: the creature left combat alive.
    Escaped(CreatureId),
    /// The combat was won.
    Won,
    /// The combat was lost.
    Lost,
}

/// Mutable access to the state for the duration of one action.
///
/// What listens is read from the state ([`super::listeners`]): the state's own methods keep
/// the hook unions in step with every change to what listens.
#[derive(Debug)]
pub struct Engine<'a> {
    pub(crate) s: &'a mut CombatState,
    /// The event sink; `None` when events are not recorded.
    events: Option<&'a mut Vec<Event>>,
    decisions: &'a [Decision],
    next_decision: usize,
    /// `CardSelectCmd.PushSelector(new VakuuCardSelector())` is active (`WhisperingEarring`): card
    /// choices pick the first options instead of asking.
    pub(crate) vakuu_selector: bool,
    /// A preview runs this engine: a draw or a shuffle stops it with [`Interrupt::Hidden`]
    /// before it moves a card.
    pub(crate) stop_at_hidden: bool,
}

impl<'a> Engine<'a> {
    pub(crate) const fn new(
        s: &'a mut CombatState,
        events: Option<&'a mut Vec<Event>>,
        decisions: &'a [Decision],
    ) -> Self {
        Self {
            s,
            events,
            decisions,
            next_decision: 0,
            vakuu_selector: false,
            stop_at_hidden: false,
        }
    }

    /// Whether no listener overrides `hook`: a dispatch of `hook` would visit nobody.
    #[inline]
    pub(crate) fn lacks(&self, hook: Hook) -> bool {
        self.s.lacks(hook)
    }

    /// The state as [`Hooked`].
    #[inline]
    pub(crate) const fn hooked(&self) -> Hooked<'_> {
        self.s.hooked()
    }

    pub(crate) fn emit(&mut self, event: Event) {
        if let Some(events) = self.events.as_deref_mut() {
            events.push(event);
        }
    }

    /// [`Interrupt::Hidden`] if a preview runs this engine ([`Engine::stop_at_hidden`]).
    #[inline]
    pub(crate) fn check_hidden(&self) -> Flow {
        if self.stop_at_hidden {
            return Err(Box::new(Interrupt::Hidden));
        }
        Ok(())
    }

    /// Whether events are recorded: skip building an expensive event when not.
    pub(crate) const fn records_events(&self) -> bool {
        self.events.is_some()
    }

    /// Asks the player for a decision: the next logged answer, or an interrupt. The log holds
    /// whole answers: [`Combat`](super::Combat) folds picks into [`Decision::Cards`].
    fn decide(&mut self, request: DecisionRequest) -> Flow<Decision> {
        let Some(decision) = self.decisions.get(self.next_decision) else {
            return Err(Box::new(Interrupt::Decision(request)));
        };
        self.next_decision += 1;
        Ok(decision.clone())
    }

    /// `VakuuCardSelector.GetSelectedCards` (`VakuuCardSelector.cs:16`): the first `max` options.
    /// `FromCombatPile` sorts a draw pile's cards by rarity, then id, before asking the selector
    /// (`CardSelectCmd.cs:487-495`), so options that all sit in the draw pile are sorted so.
    fn vakuu_select(&self, mut options: Vec<CardId>, max: usize) -> Vec<CardId> {
        if options
            .iter()
            .all(|&c| self.s.pile_of(c) == Some(PileType::Draw))
        {
            options.sort_by(|&a, &b| {
                let (a, b) = (self.s.card(a), self.s.card(b));
                a.kind
                    .rarity()
                    .cmp(&b.kind.rarity())
                    .then_with(|| a.kind.entry().cmp(b.kind.entry()))
            });
        }
        options.truncate(max);
        options
    }

    /// `CardSelectCmd.FromHand`/`FromCombatPile` shape: choose between `min` and `max` of
    /// `options`. Skips the prompt when combat is ending, when there are no options, or when
    /// an exact count leaves no freedom (`CardSelectCmd.cs:390,451,479-483,819-840`).
    pub(crate) fn choose_cards(
        &mut self,
        prompt: Prompt,
        options: Vec<CardId>,
        min: usize,
        max: usize,
    ) -> Flow<Vec<CardId>> {
        if self.hooked().is_over_or_ending() || options.is_empty() {
            return Ok(Vec::new());
        }
        let require_manual_confirmation = min != max;
        if !require_manual_confirmation && options.len() <= min {
            return Ok(options);
        }
        if self.vakuu_selector {
            return Ok(self.vakuu_select(options, max));
        }
        // A whole answer (`Decision::Cards`) needs `min` options; every caller has `min == max`
        // or `min == 0` (core-combat review #7).
        debug_assert!(
            min <= options.len(),
            "a selection of {min} to {max} from {} options",
            options.len()
        );
        let request = DecisionRequest::ChooseCards {
            options,
            min,
            max,
            picked: Vec::new(),
            prompt,
        };
        match self.decide(request)? {
            Decision::Cards(cards) => Ok(cards),
            Decision::Pick(_) | Decision::Unpick(_) | Decision::Done => {
                unreachable!("picks reach the decision log folded into `Cards`")
            }
        }
    }
}
