//! [`RunCtx`]: the run as relic hooks and event callbacks see it, and the player prompts they
//! can open.

use super::cards::RunCard;
use super::rewards::{self, CardCreation, PendingReward, Reward};
use super::state::RunState;
use super::{CardSelect, Run, RunError, SelectPurpose};
use crate::combat::Encounter;
use crate::content::relics::{RelicData, RelicKind};

/// A player choice a hook or callback opens; it runs once the hook returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Prompt {
    /// `RewardsCmd.OfferCustom`.
    Rewards(Vec<Reward>),
    /// A card selection.
    Select(CardSelect),
    /// `CardSelectCmd.FromChooseABundleScreen`.
    Bundles(Vec<Vec<RunCard>>),
    /// `EnterCombatWithoutExitingEvent`: a fight in the event's room.
    Combat {
        encounter: Encounter,
        extra_rewards: Vec<PendingReward>,
        resume: bool,
    },
}

/// What runs once the prompt a hook opened has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AfterPrompt {
    /// `NeowsBones`: this many curses join the deck (`Rng.Niche`).
    Curses(usize),
    /// `PreservedFog`: this card joins the deck (`CardPileCmd.AddCurseToDeck`).
    AddCard(&'static str),
}

/// The run as a relic hook or an event callback sees it. Mutate the run through
/// [`RunCtx::state`] (the run commands are [`RunState`] methods) and the methods here.
#[derive(Debug)]
pub struct RunCtx<'a> {
    pub(super) run: &'a mut Run,
    pub(super) prompt: Option<Prompt>,
    pub(super) after: Option<AfterPrompt>,
}

impl<'a> RunCtx<'a> {
    pub(super) const fn new(run: &'a mut Run) -> Self {
        Self {
            run,
            prompt: None,
            after: None,
        }
    }

    /// The run state.
    pub const fn state(&mut self) -> &mut RunState {
        &mut self.run.state
    }

    /// `RelicCmd.Obtain`: the relic joins, leaves both bags, and its `AfterObtained` runs. A
    /// prompt it opens becomes this context's prompt.
    ///
    /// # Errors
    /// [`RunError::UnsupportedRelic`] if the relic is not ported, or if it prompts while this
    /// context already has a prompt.
    pub fn obtain_relic(&mut self, id: &'static str) -> Result<(), RunError> {
        self.obtain_preset_relic(id, None)
    }

    /// [`RunCtx::obtain_relic`] of a relic whose fields were set before it is obtained
    /// (`SeaGlass.CharacterId`, `SetupForPlayer`); `None` is its field initializers.
    ///
    /// # Errors
    /// As [`RunCtx::obtain_relic`].
    pub fn obtain_preset_relic(
        &mut self,
        id: &'static str,
        data: Option<RelicData>,
    ) -> Result<(), RunError> {
        self.obtain_relic_as(id, data, false)
    }

    /// [`RunCtx::obtain_relic`] of a relic made wax first (`IsWax = true`, `ToyBox`'s rewards).
    ///
    /// # Errors
    /// As [`RunCtx::obtain_relic`].
    pub fn obtain_wax_relic(&mut self, id: &'static str) -> Result<(), RunError> {
        self.obtain_relic_as(id, None, true)
    }

    /// [`RunCtx::obtain_preset_relic`] with `IsWax` set to `wax` before the relic joins.
    pub(super) fn obtain_relic_as(
        &mut self,
        id: &'static str,
        data: Option<RelicData>,
        wax: bool,
    ) -> Result<(), RunError> {
        let kind = RelicKind::from_id(id)
            .filter(|k| k.model().obtainable())
            .ok_or(RunError::UnsupportedRelic(id))?;
        self.obtain(kind, None, data, wax).map_err(|e| match e {
            RunError::IllegalAction => RunError::UnsupportedRelic(id),
            e => e,
        })
    }

    /// `RelicCmd.Obtain(relic, player, index)`: at `index` of the relic list, else at its end.
    fn obtain(
        &mut self,
        kind: RelicKind,
        index: Option<usize>,
        data: Option<RelicData>,
        wax: bool,
    ) -> Result<(), RunError> {
        let s = &mut self.run.state;
        let me = index.unwrap_or(s.relics.len());
        let mut relic = super::RunRelic::new(kind);
        if let Some(data) = data {
            relic.data = data;
        }
        relic.wax = wax;
        s.relics.insert(me, relic);
        s.player_relic_bag.remove(kind.id());
        s.shared_relic_bag.remove(kind.id());
        let mut inner = RunCtx::new(self.run);
        kind.model().after_obtained(&mut inner, me)?;
        if let Some(prompt) = inner.prompt {
            if self.prompt.is_some() {
                return Err(RunError::IllegalAction);
            }
            self.prompt = Some(prompt);
            self.after = inner.after;
        } else if let Some(after) = inner.after {
            self.run.run_after_prompt(after);
        }
        Ok(())
    }

    /// `RunManager.Instance.GenerateMap()` from a hook (`GoldenCompass.AfterObtained`): the
    /// current act's map is generated again, through `ModifyGeneratedMap`
    /// ([`RunState::generate_map`]). The player stays on the current point; visited points the
    /// new map lacks are forgotten.
    ///
    /// # Errors
    /// [`RunError::Map`] where the game throws.
    pub fn regenerate_map(&mut self) -> Result<(), RunError> {
        self.run.state.generate_map().map_err(RunError::Map)
    }

    /// Adds `count` curses of the `CurseCardPool` (those modifiers may generate) to the deck on
    /// the run's `Niche` stream once the prompt this hook opens ends, or at once if it opens
    /// none (`NeowsBones`).
    pub const fn then_add_curses(&mut self, count: usize) {
        self.after = Some(AfterPrompt::Curses(count));
    }

    /// Adds a new `id` card to the deck (`CardPileCmd.Add`) once the prompt this hook opens
    /// ends, or at once if it opens none (`PreservedFog`'s `Folly` after its removal screen).
    pub const fn then_add_card(&mut self, id: &'static str) {
        self.after = Some(AfterPrompt::AddCard(id));
    }

    /// `RelicCmd.Remove`: the relic at `index` leaves the relic list. `AfterRemoved` has no
    /// in-scope override.
    pub fn remove_relic(&mut self, index: usize) {
        self.run.state.relics.remove(index);
    }

    /// `RelicCmd.Replace(original, replacement)`: the relic at `index` is removed and
    /// `replacement` obtained in its place.
    ///
    /// # Errors
    /// As [`RunCtx::obtain_relic`].
    pub fn replace_relic(
        &mut self,
        index: usize,
        replacement: &'static str,
    ) -> Result<(), RunError> {
        let kind = RelicKind::from_id(replacement)
            .filter(|k| k.model().obtainable())
            .ok_or(RunError::UnsupportedRelic(replacement))?;
        self.remove_relic(index);
        self.obtain(kind, Some(index), None, false)
            .map_err(|e| match e {
                RunError::IllegalAction => RunError::UnsupportedRelic(replacement),
                e => e,
            })
    }

    /// `RelicFactory.PullNextRelicFromFront(Owner)`.
    pub fn pull_relic_from_front(&mut self) -> &'static str {
        rewards::pull_relic_from_front(&mut self.run.state)
    }

    /// `CardFactory.CreateForReward(Owner, count, options)`.
    pub fn create_cards(&mut self, count: usize, options: CardCreation) -> Vec<RunCard> {
        rewards::create_card_reward(&mut self.run.state, count, options)
    }

    /// `new CardReward(options, count, Owner)`, populated (`CardFactory.CreateForReward`).
    pub fn card_reward(&mut self, count: usize, options: CardCreation) -> Reward {
        Reward::Card(rewards::CardReward::generate(
            &mut self.run.state,
            count,
            options,
        ))
    }

    /// `RewardsCmd.OfferCustom(Owner, rewards)` with populated rewards: a rewards screen once the
    /// hook returns (`ModifyRewards` runs with no room). `RewardsSet.Offer` does nothing for a
    /// dead player.
    pub fn offer_rewards(&mut self, rewards: Vec<Reward>) {
        if self.run.state.hp <= 0 {
            return;
        }
        let rewards = rewards::custom_rewards(&mut self.run.state, rewards);
        self.set_prompt(Prompt::Rewards(rewards));
    }

    /// `SelectCardsToAddToDeckFromGrid(cards, prefs(count))`: the player picks `count` of the
    /// cards (all of them, without a prompt, if there are no more than `count`) and they join
    /// the deck in pick order.
    pub fn select_cards_to_add(&mut self, cards: Vec<RunCard>, count: usize) {
        if cards.len() <= count {
            for card in cards {
                self.run.state.add_card(card);
            }
            return;
        }
        self.set_prompt(Prompt::Select(CardSelect {
            purpose: SelectPurpose::AddToDeck,
            options: (0..cards.len()).collect(),
            offered: cards,
            count,
            picked: Vec::new(),
        }));
    }

    /// `CardSelectCmd.FromSimpleGridForRewards(cards, prefs(0, cards.Count))` (`SeaGlass`): the
    /// player picks any number of the cards, then confirms; they join the deck in pick order.
    pub fn select_cards_to_add_up_to(&mut self, cards: Vec<RunCard>) {
        if cards.is_empty() {
            return;
        }
        self.set_prompt(Prompt::Select(CardSelect {
            purpose: SelectPurpose::AddToDeckUpTo,
            options: (0..cards.len()).collect(),
            count: cards.len(),
            offered: cards,
            picked: Vec::new(),
        }));
    }

    /// `CardSelectCmd.FromDeck*` with `MinSelect == MaxSelect == count` and no manual
    /// confirmation: the player picks `count` of the deck cards `options` (deck indices, in the
    /// order the screen lists them; build them with [`RunState::upgradable_cards`],
    /// [`RunState::removable_cards`], [`RunState::transformable_cards`] or
    /// [`RunState::enchantable_cards`]), then `purpose` acts on them. With no more options than
    /// `count` they are all taken without a prompt.
    ///
    /// # Errors
    /// As the purpose's command when it runs at once (an unported transform).
    ///
    /// # Panics
    /// For [`SelectPurpose::Event`] (events use `EventCtx::select_deck_cards`) and
    /// [`SelectPurpose::ChooseACard`] (use [`RunCtx::choose_a_card`]).
    pub fn select(
        &mut self,
        purpose: SelectPurpose,
        options: Vec<usize>,
        count: usize,
    ) -> Result<(), RunError> {
        assert!(
            !matches!(
                purpose,
                SelectPurpose::Event { .. } | SelectPurpose::ChooseACard { .. }
            ),
            "an event's own selection goes through EventCtx"
        );
        let select = CardSelect {
            purpose,
            options,
            offered: Vec::new(),
            count,
            picked: Vec::new(),
        };
        if select.options.len() <= count {
            return self.run.resolve_deck_purpose(purpose, &select.options);
        }
        self.set_prompt(Prompt::Select(select));
        Ok(())
    }

    /// `CardSelectCmd.FromDeck*` with `MinSelect` 0, `MaxSelect == count` and manual
    /// confirmation (`Kifuda`, `GnarledHammer`): the player picks up to `count` of the deck cards
    /// `options`, then confirms; `purpose` acts on them. No options, no prompt.
    pub fn select_up_to(&mut self, purpose: SelectPurpose, options: Vec<usize>, count: usize) {
        if options.is_empty() {
            return;
        }
        self.set_prompt(Prompt::Select(CardSelect {
            purpose,
            options,
            offered: Vec::new(),
            count,
            picked: Vec::new(),
        }));
    }

    /// `CardSelectCmd.FromChooseACardScreen(cards, canSkip: true)`: the player picks one of
    /// `cards` or skips; the pick joins the deck (`CardPileCmd.Add`), then `extra`, if any.
    /// With no cards nothing is shown (`ReportSoftlock`) and only `extra` joins.
    pub fn choose_a_card(&mut self, cards: Vec<RunCard>, extra: Option<&'static str>) {
        if cards.is_empty() {
            if let Some(id) = extra {
                self.run.state.add_card(RunCard::new(id));
            }
            return;
        }
        self.set_prompt(Prompt::Select(CardSelect {
            purpose: SelectPurpose::ChooseACard { extra },
            options: (0..cards.len()).collect(),
            offered: cards,
            count: 1,
            picked: Vec::new(),
        }));
    }

    /// `CardSelectCmd.FromChooseABundleScreen(player, bundles)` (`CardSelectCmd.cs:965-990`), then
    /// `CardPileCmd.Add` of each card of the chosen bundle. In test mode the game takes
    /// `bundles[0]` without a screen.
    pub fn choose_a_bundle(&mut self, bundles: Vec<Vec<RunCard>>) {
        if self.run.state.test_mode {
            for &card in &bundles[0] {
                self.run.state.add_card(card);
            }
            return;
        }
        self.set_prompt(Prompt::Bundles(bundles));
    }

    /// `RewardsCmd.OfferCustom(Owner, rewards)` for rewards populated now, in order (each
    /// `Reward.Populate` draws), then as [`RunCtx::offer_rewards`].
    pub fn offer_pending_rewards(&mut self, rewards: Vec<PendingReward>) {
        if self.run.state.hp <= 0 {
            return;
        }
        let rewards = rewards
            .into_iter()
            .map(|p| p.populate(&mut self.run.state))
            .collect();
        self.offer_rewards(rewards);
    }

    pub(super) fn set_prompt(&mut self, prompt: Prompt) {
        assert!(self.prompt.is_none(), "a hook opens at most one prompt");
        self.prompt = Some(prompt);
    }
}
