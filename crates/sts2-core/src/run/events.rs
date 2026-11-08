//! The event framework (`Models/EventModel.cs`, `Rooms/EventRoom.cs`): picking the next event
//! of a `?` room, an event room's pages, and [`EventCtx`], the run API event callbacks use.
//!
//! The events themselves are content (`content/events/`), each with its own `IsAllowed` (act
//! gates included); the act lists in `run/rooms.rs` hold every event, ported or not, so porting
//! an event is only its file. An unported event is refused ([`is_allowed`]); if it is still picked
//! (every event refused), its room is a stub ([`super::RunDecision::Unsupported`]).

use std::ops::{Deref, DerefMut};

use super::ctx::{Prompt, RunCtx};
use super::rewards::PendingReward;
use super::state::{Room, RunState};
use super::{CardSelect, Phase, Run, RunError, SelectPurpose, hooks};
use crate::combat::{Encounter, PromptText};
use crate::content::encounters::EncounterData;
use crate::content::events::{EventData, EventKind};
use crate::rng::{Rng, xxh64};

/// What an option was built with: the callback (`tag`, the event's own name for it) and an
/// argument (`TheFutureOfPotions`: the potion's belt slot).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventAction {
    /// The callback's name.
    pub tag: &'static str,
    /// Its argument.
    pub arg: usize,
}

/// An `EventOption`: its `TextKey` and callback; `None` for a locked option (`onChosen` null).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventOption {
    /// `TextKey`.
    pub text_key: String,
    /// The callback, `None` for a locked option.
    pub action: Option<EventAction>,
}

impl EventOption {
    /// An option with the callback `tag`.
    #[must_use]
    pub fn new(text_key: impl Into<String>, tag: &'static str) -> Self {
        Self::with_arg(text_key, tag, 0)
    }

    /// An option with the callback `tag` and an argument.
    #[must_use]
    pub fn with_arg(text_key: impl Into<String>, tag: &'static str, arg: usize) -> Self {
        Self {
            text_key: text_key.into(),
            action: Some(EventAction { tag, arg }),
        }
    }

    /// A locked option (`IsLocked`): shown, never chosen.
    #[must_use]
    pub fn locked(text_key: impl Into<String>) -> Self {
        Self {
            text_key: text_key.into(),
            action: None,
        }
    }
}

/// What an option callback ends with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Page {
    /// `SetEventState(description, options)`: a new page.
    Next(Vec<EventOption>),
    /// `SetEventFinished(description)`: no options; leaving goes to the map. Holds the
    /// description's `events` key (`WELLSPRING.pages.BATHE.description`), empty for
    /// [`Page::left`].
    Finished(String),
}

impl Page {
    /// [`Page::Finished`] with the description of `event`'s `page`:
    /// `{event}.pages.{page}.description`.
    #[must_use]
    pub fn finished(event: &str, page: &str) -> Self {
        Self::Finished(format!("{event}.pages.{page}.description"))
    }

    /// A callback that leaves the event without `SetEventFinished` (a fight that takes the
    /// room, `shouldResumeAfterCombat: false`): finished with no description (an empty key).
    #[must_use]
    pub const fn left() -> Self {
        Self::Finished(String::new())
    }
}

/// A started event (the mutable `EventModel`): its `Rng`, vars and current options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRoom {
    /// The event.
    pub kind: EventKind,
    /// `EventModel.Rng`.
    pub rng: Rng,
    /// Its vars and fields.
    pub data: EventData,
    /// `CurrentOptions` (empty once finished).
    pub options: Vec<EventOption>,
    /// `IsFinished`: the finished page's description key ([`Page::Finished`]).
    pub finished: Option<String>,
    /// The rest of a callback that waits for its prompts ([`EventCtx::then`]).
    pub then: Option<EventAction>,
    /// The fight the event resumes after ([`EventModel::resume`]), until it has resumed.
    ///
    /// [`EventModel::resume`]: crate::content::events::EventModel
    pub exited: Option<ExitedCombat>,
}

/// The `CombatRoom` an event resumes after (`EventModel.Resume(exitedRoom)`).
///
/// Its encounter, the encounter's fields at the end of the fight
/// (`BattlewornDummyEventEncounter.RanOutOfTime`) and its `ExtraRewards` (the event's, then those
/// added during the fight).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitedCombat {
    /// The encounter fought.
    pub encounter: Encounter,
    /// Its fields.
    pub data: EncounterData,
    /// The rewards the event added.
    pub extra_rewards: Vec<PendingReward>,
}

/// The run as event callbacks see it: a [`RunCtx`] (the run commands, relics, rewards and deck
/// selections) plus the event's own `Rng` and vars.
#[derive(Debug)]
pub struct EventCtx<'a> {
    ctx: RunCtx<'a>,
    kind: EventKind,
    /// The event's own `Rng` (`runSeed + slot + xxh64(Id.Entry)`).
    pub rng: &'a mut Rng,
    /// The event's vars.
    pub data: &'a mut EventData,
    then: &'a mut Option<EventAction>,
}

impl<'a> Deref for EventCtx<'a> {
    type Target = RunCtx<'a>;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl DerefMut for EventCtx<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

impl EventCtx<'_> {
    /// `CardSelectCmd.FromDeck*` for the event's own use of the picked cards: the player picks
    /// `count` of the deck cards `options` (see [`RunCtx::select`]), then the event's
    /// [`EventModel::on_cards_selected`](crate::content::events::EventModel) runs with `action`
    /// and the picked deck indices in pick order. With no more options than `count` it runs at
    /// once with all of them. `text` is the selection's prompt (`CardSelectorPrefs.Prompt`).
    ///
    /// # Errors
    /// What `on_cards_selected` returns when it runs at once.
    pub fn select_deck_cards(
        &mut self,
        action: EventAction,
        text: PromptText,
        options: Vec<usize>,
        count: usize,
    ) -> Result<(), RunError> {
        if options.len() <= count {
            return self.kind.model().on_cards_selected(action, self, &options);
        }
        self.ctx.set_prompt(Prompt::Select(CardSelect {
            purpose: SelectPurpose::Event { action, text },
            options,
            offered: Vec::new(),
            count,
            picked: Vec::new(),
        }));
        Ok(())
    }

    /// The callback goes on after its prompts: once the card selection or rewards screen it opened
    /// (if any) is done, [`EventModel::after_prompt`](crate::content::events::EventModel) runs
    /// with `action` and returns the page. The callback returns a placeholder page meanwhile.
    pub const fn then(&mut self, action: EventAction) {
        *self.then = Some(action);
    }

    /// `CardCmd.TransformToRandom(card, Rng)` on the deck card at `index`, drawing on the
    /// event's `Rng` ([`RunState::transform_to_random`]).
    ///
    /// # Errors
    /// [`RunError::UnsupportedCard`] if the card's pool is not modeled.
    pub fn transform_to_random(&mut self, index: usize) -> Result<(), RunError> {
        self.ctx.run.state.transform_to_random(index, self.rng)
    }

    /// `EnterCombatWithoutExitingEvent(encounter, extraRewards, shouldResumeAfterCombat)`
    /// (`Models/EventModel.cs:556-586`, `Multiplayer.Game/EventCombatSynchronizer.cs`): once the
    /// callback returns, a fight against `encounter` in a `CombatRoom` pushed on the event's
    /// room; its rewards are the encounter room's plus `extra_rewards`. Leaving the rewards goes
    /// back to the event if `resume`, else to the map.
    ///
    /// # Errors
    /// [`RunError::UnsupportedEncounter`] if the encounter is not ported.
    pub fn enter_combat(
        &mut self,
        encounter: &'static str,
        extra_rewards: Vec<PendingReward>,
        resume: bool,
    ) -> Result<(), RunError> {
        let encounter =
            Encounter::from_entry(encounter).ok_or(RunError::UnsupportedEncounter(encounter))?;
        self.ctx.set_prompt(Prompt::Combat {
            encounter,
            extra_rewards,
            resume,
        });
        Ok(())
    }
}

/// `EventModel.BeginEvent`'s `Rng`: `runSeed + slotIndex + xxh64(Id.Entry)` (slot 0 in single
/// player, and shared events use 0).
#[must_use]
pub fn event_rng(run_seed: u64, entry: &str) -> Rng {
    Rng::new(run_seed.wrapping_add(xxh64(entry)))
}

/// `IsAllowed(runState)` of an event in any act's list, ported or not.
///
/// A ported event answers from its own file ([`EventModel::is_allowed`]), act gates included
/// (`CurrentActIndex` is [`RunState::act_index`]), so porting an event is only its file. An
/// unported event is allowed (`EventModel.IsAllowed` is `true` unless overridden,
/// `EventModel.cs:324-327`; no unported event overrides it): the pull must pick it where the game does
/// (its room is then a stub the diff policy never enters), not skip to the next event (PX15:
/// Glory's `RoundTeaParty` was skipped, so the engine offered the event after it).
///
/// [`EventModel::is_allowed`]: crate::content::events::EventModel
#[must_use]
pub fn is_allowed(id: &str, s: &RunState) -> bool {
    event_kind(id).is_none_or(|kind| kind.model().is_allowed(s))
}

/// The ported event with `ModelId.Entry` `id`; tests also see the test-only events.
fn event_kind(id: &str) -> Option<EventKind> {
    #[cfg(test)]
    if let Some(kind) = EventKind::TEST_ONLY.into_iter().find(|k| k.id() == id) {
        return Some(kind);
    }
    EventKind::from_id(id)
}

impl Run {
    /// A debug hook for differential tests: `RunManager.EnterRoom(new EventRoom(kind))` while the
    /// run waits on the map, as the referee's `enter_room` does (`RunManager.cs:1169-1237`). The
    /// map room is exited, `BeforeRoomEntered` runs (for the point the run stands on), the event
    /// begins and `MarkRoomVisited(Event)` counts it. Unlike a `?` room nothing is pulled from the
    /// act's list or added to the visited events, no history entry or floor is added, and the
    /// event's `IsAllowed` is not checked. A finished event leaves for the map as usual.
    ///
    /// # Errors
    /// [`RunError::IllegalAction`] unless the run is on the map.
    pub fn debug_enter_event(&mut self, kind: EventKind) -> Result<(), RunError> {
        self.forget_journal();
        if self.phase != Phase::Map {
            return Err(RunError::IllegalAction);
        }
        if let Some(point) = self
            .state
            .coord()
            .and_then(|c| self.state.map.point(c))
            .map(|p| p.kind)
        {
            hooks::before_room_entered(&mut self.state, point);
        }
        self.finished_event = None;
        self.state.room = Room::Event { id: kind.id() };
        self.phase = self.begin_event(kind)?;
        self.state.act_mut().events_visited += 1;
        Ok(())
    }

    /// A debug hook for differential tests: `RunManager.EnterAct(index, doTransition: false)`
    /// while the run waits on the map (`RunManager.cs:1321-1350`), as the referee's
    /// `run_loadout` `"act_index"` does: the act is entered with [`Self::enter_act`] (rooms
    /// exited, visited coords cleared, map generated) and the run waits on the new act's map with
    /// no point visited. Lets an act-gated event be forced in the act it belongs to.
    ///
    /// # Errors
    /// [`RunError::IllegalAction`] unless the run is on the map and `index` is a later act.
    pub fn debug_enter_act(&mut self, index: usize) -> Result<(), RunError> {
        self.forget_journal();
        if self.phase != Phase::Map
            || index <= self.state.act_index
            || index >= self.state.acts.len()
        {
            return Err(RunError::IllegalAction);
        }
        self.enter_act(index)
    }

    /// `ActModel.PullNextEvent` (`ActModel.cs:437-443`, `RoomSet.cs:110-125`): skip events that
    /// are not allowed or already visited (each skip counts as visited; if all fail, the
    /// original one repeats), then `AddVisitedEvent`. `ModifyNextEvent` has no Act 1 listener.
    pub(super) fn pull_next_event(&mut self) -> &'static str {
        let s = &self.state;
        let act = s.act();
        let n = act.events.len();
        let mut skipped = 0;
        for _ in 0..n {
            let next = act.events[(act.events_visited + skipped) % n];
            if is_allowed(next, s) && !s.visited_event_ids.contains(&next) {
                break;
            }
            skipped += 1;
        }
        let act = self.state.act_mut();
        act.events_visited += skipped;
        let id = act.events[act.events_visited % n];
        let id = hooks::modify_next_event(&self.state, id);
        self.state.visited_event_ids.push(id);
        id
    }

    /// `EventRoom.EnterInternal` → `BeginEvent` for a ported event: its `Rng`, then
    /// `BeforeEventStarted`, `CalculateVars` and the initial options, then `AfterRoomEntered`
    /// (`Rooms/EventRoom.cs:56-88`).
    pub(super) fn begin_event(&mut self, kind: EventKind) -> Result<Phase, RunError> {
        let mut rng = event_rng(self.state.rng.seed(), kind.id());
        let mut data = EventData::None;
        let mut then = None;
        let mut ctx = EventCtx {
            ctx: RunCtx::new(self),
            kind,
            rng: &mut rng,
            data: &mut data,
            then: &mut then,
        };
        let options = kind.model().begin(&mut ctx)?;
        debug_assert!(
            ctx.ctx.prompt.is_none(),
            "events do not prompt before an option"
        );
        // `EventRoom.EnterInternal`: `AfterRoomEntered` once the event has begun.
        hooks::after_room_entered(&mut self.state, super::RoomType::Event);
        Ok(Phase::Event(Box::new(EventRoom {
            kind,
            rng,
            data,
            options,
            finished: None,
            then: None,
            exited: None,
        })))
    }

    /// `EventOption.Chosen()`: the callback runs, then the event shows its next page, opens its
    /// prompt, or (finished) is left for the map, as sts2-cli does at once. A player killed by
    /// the callback ends the run.
    pub(super) fn choose_event_option(&mut self, index: usize) -> Result<(), RunError> {
        let Phase::Event(room) = &self.phase else {
            return Err(RunError::IllegalAction);
        };
        let mut room = room.clone();
        let action = room
            .options
            .get(index)
            .and_then(|o| o.action)
            .ok_or(RunError::IllegalAction)?;
        let mut ctx = EventCtx {
            ctx: RunCtx::new(self),
            kind: room.kind,
            rng: &mut room.rng,
            data: &mut room.data,
            then: &mut room.then,
        };
        let page = room.kind.model().choose(action, &mut ctx)?;
        let prompt = ctx.ctx.prompt;
        match page {
            Page::Next(options) => room.options = options,
            Page::Finished(key) => {
                room.options.clear();
                room.finished = Some(key);
            }
        }
        self.after_event_callback(room, prompt)
    }

    /// The end of an event callback: death ends the run on the event's page, a prompt opens with
    /// the event waiting behind it, else the event goes on.
    pub(super) fn after_event_callback(
        &mut self,
        room: Box<EventRoom>,
        prompt: Option<Prompt>,
    ) -> Result<(), RunError> {
        if self.state.hp <= 0 {
            self.suspended_event = Some(room);
            self.phase = Phase::GameOver;
            return Ok(());
        }
        match prompt {
            Some(Prompt::Combat {
                encounter,
                extra_rewards,
                resume,
            }) => {
                self.suspended_event = resume.then_some(room);
                self.enter_event_combat(encounter, extra_rewards)?;
            }
            Some(prompt) => {
                self.suspended_event = Some(room);
                self.phase = match prompt {
                    Prompt::Rewards(rewards) => Phase::Rewards(rewards),
                    Prompt::Select(select) => Phase::CardSelect(select),
                    Prompt::Bundles(bundles) => Phase::Bundles(bundles),
                    Prompt::Combat { .. } => unreachable!("handled above"),
                };
            }
            None => self.resume_event(room),
        }
        Ok(())
    }

    /// An event's deck selection is done: its `on_cards_selected` runs with the picked cards.
    pub(super) fn resume_event_selection(
        &mut self,
        action: EventAction,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let mut room = self
            .suspended_event
            .take()
            .expect("an event selection has its event waiting");
        let mut ctx = EventCtx {
            ctx: RunCtx::new(self),
            kind: room.kind,
            rng: &mut room.rng,
            data: &mut room.data,
            then: &mut room.then,
        };
        room.kind
            .model()
            .on_cards_selected(action, &mut ctx, cards)?;
        let prompt = ctx.ctx.prompt;
        self.after_event_callback(room, prompt)
    }

    /// Back to the event after its prompt: its page, or the map once it is finished. After a
    /// fight (`RunManager.ResumePreviousRoom` → `EventRoom.Resume`) the event's `Resume` runs
    /// first ([`EventModel::resume`]); a prompt it opens comes before the page.
    ///
    /// [`EventModel::resume`]: crate::content::events::EventModel
    pub(super) fn resume_event(&mut self, mut room: Box<EventRoom>) {
        if let Some(exited) = room.exited.take() {
            // `ExitCurrentRoom` pops the `CombatRoom` before `Resume`, so what the event offers
            // there (`BattlewornDummy`'s rewards, `TheHunt`'s card) is offered in its own room.
            self.state.room = Room::Event { id: room.kind.id() };
            let mut ctx = EventCtx {
                ctx: RunCtx::new(self),
                kind: room.kind,
                rng: &mut room.rng,
                data: &mut room.data,
                then: &mut room.then,
            };
            let page = room.kind.model().resume(&exited, &mut ctx);
            let prompt = ctx.ctx.prompt;
            match page {
                Some(Page::Next(options)) => room.options = options,
                Some(Page::Finished(key)) => {
                    room.options.clear();
                    room.finished = Some(key);
                }
                None => {}
            }
            if prompt.is_some() {
                self.after_event_callback(room, prompt)
                    .expect("a resumed event does not start a fight");
                return;
            }
        }
        if let Some(action) = room.then.take() {
            let mut ctx = EventCtx {
                ctx: RunCtx::new(self),
                kind: room.kind,
                rng: &mut room.rng,
                data: &mut room.data,
                then: &mut room.then,
            };
            let page = room.kind.model().after_prompt(action, &mut ctx);
            debug_assert!(
                ctx.ctx.prompt.is_none(),
                "the end of a callback does not prompt"
            );
            match page {
                Page::Next(options) => room.options = options,
                Page::Finished(key) => {
                    room.options.clear();
                    room.finished = Some(key);
                }
            }
        }
        if room.finished.is_some() {
            self.finished_event = Some(room);
            self.leave_room();
        } else {
            self.state.room = Room::Event { id: room.kind.id() };
            self.phase = Phase::Event(room);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Act1, RoomType, RunAction, RunDecision, UnlockState};
    use super::*;
    use crate::content::cards::CardType;
    use crate::content::characters::Character;
    use crate::content::enchantments::{Enchantment, EnchantmentKind};
    use crate::content::relics::RelicKind;
    use crate::run::Reward;
    use crate::run::cards::RunCard;
    use crate::run::odds::CardRarity;
    use crate::run::relics::RunRelic;

    fn t6s0() -> Run {
        Run::new(
            Character::Necrobinder,
            "T6S0",
            0,
            UnlockState::all(),
            Act1::Overgrowth,
        )
        .expect("T6S0 starts")
    }

    /// `T6S0` after Neow, standing in `kind` (its `IsAllowed` is not checked).
    fn in_event(kind: EventKind, set_up: impl FnOnce(&mut RunState)) -> Run {
        let mut run = t6s0();
        set_up(&mut run.state);
        run.state.room = Room::Event { id: kind.id() };
        run.phase = run.begin_event(kind).expect("the event begins");
        run
    }

    fn page(run: &Run) -> Vec<String> {
        match run.pending_decision() {
            RunDecision::Event(room) => room.options.iter().map(|o| o.text_key.clone()).collect(),
            other => panic!("not on an event page: {other:?}"),
        }
    }

    /// C27: `EventRoom::vars` gives the canonical vars, the rolled ones and model titles.
    #[test]
    fn event_vars_show_rolled_and_canonical_values() {
        use crate::content::events::EventVar;
        use crate::content::tables::VarText;
        use crate::decimal::Decimal;
        let run = in_event(EventKind::JungleMazeAdventure, |_| {});
        let room = run.event().expect("in the event");
        let vars = room.vars(&run.state);
        let EventData::JungleMaze { solo_gold, .. } = room.data else {
            panic!("JungleMaze data");
        };
        assert!(vars.contains(&("SoloGold", EventVar::Number(solo_gold))));
        // `DamageVar("SoloHp", 18m, ...)` is canonical.
        assert!(vars.contains(&("SoloHp", EventVar::Number(Decimal::from(18)))));
        let run = in_event(EventKind::RoundTeaParty, |_| {});
        let room = run.event().expect("in the event");
        assert!(
            room.vars(&run.state)
                .contains(&("Relic", EventVar::Text(VarText::Relic("ROYAL_POISON"))))
        );
    }

    #[test]
    fn pick_skips_refused_events() {
        // `T6S0`'s list starts `BRAIN_LEECH, WAR_HISTORIAN_REPY, JUNGLE_MAZE_ADVENTURE` and Neow
        // counted one visit; `WarHistorianRepy` is never allowed. The referee's first `?` event
        // on this seed is `JungleMazeAdventure` with `events_visited` 3 once entered.
        let mut run = t6s0();
        assert_eq!(run.pull_next_event(), "JUNGLE_MAZE_ADVENTURE");
        assert_eq!(run.state.act().events_visited, 2);
        assert_eq!(run.state.visited_event_ids, ["JUNGLE_MAZE_ADVENTURE"]);
    }

    /// An act-gated event's `IsAllowed` lives in its file: the same list offers it in Act 2 and
    /// skips it in Act 1 (counting the skip as a visit).
    #[test]
    fn an_act_gated_event_is_offered_only_in_its_act() {
        let gated = EventKind::TestActGated.id();
        for (act_index, expected, visited) in [(0, "SAPPHIRE_SEED", 1), (1, gated, 0)] {
            let mut run = t6s0();
            run.state.act_index = act_index;
            let act = run.state.act_mut();
            act.events = vec![gated, "SAPPHIRE_SEED"];
            act.events_visited = 0;
            assert_eq!(run.pull_next_event(), expected, "act index {act_index}");
            assert_eq!(run.state.act().events_visited, visited);
        }
    }

    /// An unported event is pulled where the game takes it (PX15, `RL2S83` Ironclad Hive jump):
    /// Glory's `RoundTeaParty` at 12 HP or more, else skipped like any refused event.
    #[test]
    fn an_unported_event_answers_its_own_is_allowed() {
        for (hp, expected, visited) in [(89, "ROUND_TEA_PARTY", 0), (11, "SAPPHIRE_SEED", 1)] {
            let mut run = t6s0();
            run.state.hp = hp;
            let act = run.state.act_mut();
            act.events = vec!["ROUND_TEA_PARTY", "SAPPHIRE_SEED"];
            act.events_visited = 0;
            assert_eq!(run.pull_next_event(), expected, "hp {hp}");
            assert_eq!(run.state.act().events_visited, visited);
        }
        let s = &mut t6s0().state;
        assert!(!is_allowed("WAR_HISTORIAN_REPY", s));
        assert!(is_allowed("TRIAL", s));
        // `GraveOfTheForgotten` needs a deck card with a local Exhaust that is not enchanted.
        assert!(!is_allowed("GRAVE_OF_THE_FORGOTTEN", s));
        s.deck.push(RunCard::new("BOOT_SEQUENCE"));
        assert!(is_allowed("GRAVE_OF_THE_FORGOTTEN", s));
        s.deck.last_mut().expect("pushed").enchantment =
            Some(Enchantment::new(EnchantmentKind::Sharp, 1));
        assert!(!is_allowed("GRAVE_OF_THE_FORGOTTEN", s));
    }

    #[test]
    fn death_in_an_event_keeps_its_page() {
        let mut run = in_event(EventKind::JungleMazeAdventure, |s| s.hp = 10);
        run.apply(RunAction::EventOption(0)).expect("SOLO_QUEST");
        assert_eq!(run.pending_decision(), RunDecision::GameOver);
        assert_eq!(run.state.hp, 0);
        assert!(run.event().is_some_and(|e| e.finished.is_some()));
    }

    #[test]
    fn fairy_in_a_bottle_prevents_a_death_in_an_event() {
        // `CreatureCmd.Damage` → `Kill` → `Hook.ShouldDie` also runs outside combat: the Fairy
        // (a `ShouldDie` listener) goes before `LizardTail` (`ShouldDieLate`) and heals 30%.
        let mut run = in_event(EventKind::JungleMazeAdventure, |s| {
            s.hp = 10;
            s.potions[1] = Some("FAIRY_IN_A_BOTTLE");
            s.relics.push(RunRelic::new(RelicKind::LizardTail));
        });
        run.apply(RunAction::EventOption(0)).expect("SOLO_QUEST");
        assert_ne!(run.pending_decision(), RunDecision::GameOver);
        assert_eq!(run.state.hp, run.state.max_hp * 3 / 10);
        assert_eq!(run.state.potions[1], None);
        // The next death is the LizardTail's (50%), then the run ends.
        run.state.damage(run.state.hp);
        assert_eq!(run.state.hp, run.state.max_hp / 2);
        run.state.damage(run.state.hp);
        assert_eq!(run.state.hp, 0);
    }

    #[test]
    fn slippery_bridge_holds_on_with_growing_damage() {
        let mut run = in_event(EventKind::SlipperyBridge, |_| {});
        let first = run.event().map(|e| e.data.clone());
        run.apply(RunAction::EventOption(1)).expect("HOLD_ON");
        assert_eq!(run.state.hp, 66 - 3);
        assert_eq!(
            page(&run)[1],
            "SLIPPERY_BRIDGE.pages.HOLD_ON_0.options.HOLD_ON_1"
        );
        let (
            Some(EventData::SlipperyBridge { card: a, .. }),
            Some(EventData::SlipperyBridge {
                card: b, skipped, ..
            }),
        ) = (first, run.event().map(|e| e.data.clone()))
        else {
            panic!("SlipperyBridge keeps its card");
        };
        // A starter deck has no non-Basic card, so the first pick is any removable card; the
        // next one is of another kind.
        assert_ne!(run.state.deck[a].id, run.state.deck[b].id);
        assert_eq!(skipped, [a]);
        run.apply(RunAction::EventOption(1)).expect("HOLD_ON");
        assert_eq!(run.state.hp, 66 - 3 - 4);
        run.apply(RunAction::EventOption(0)).expect("OVERCOME");
        assert_eq!(run.state.deck.len(), 9);
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn tablet_of_truth_deciphers_five_times() {
        let mut run = in_event(EventKind::TabletOfTruth, |_| {});
        for (count, max_hp) in [(1, 63), (2, 57), (3, 45), (4, 21)] {
            run.apply(RunAction::EventOption(0)).expect("DECIPHER");
            assert_eq!(run.state.max_hp, max_hp);
            assert_eq!(
                page(&run)[0],
                format!("TABLET_OF_TRUTH.pages.DECIPHER_{count}.options.DECIPHER")
            );
        }
        // Four random upgrades so far (one per decipher, among the 10 upgradable cards).
        let upgraded = run
            .state
            .deck
            .iter()
            .filter(|c| c.upgrade_level > 0)
            .count();
        assert!((1..=4).contains(&upgraded));
        // The fifth costs `MaxHp - 1` and upgrades everything.
        run.apply(RunAction::EventOption(0)).expect("DECIPHER");
        assert_eq!((run.state.hp, run.state.max_hp), (1, 1));
        assert!(run.state.deck.iter().all(|c| c.upgrade_level == 1));
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn room_full_of_cheese_adds_two_picked_commons() {
        let mut run = in_event(EventKind::RoomFullOfCheese, |_| {});
        run.apply(RunAction::EventOption(0)).expect("GORGE");
        let RunDecision::CardSelect(select) = run.pending_decision() else {
            panic!("GORGE shows a grid");
        };
        assert_eq!((select.offered.len(), select.count), (8, 2));
        assert!(
            select
                .offered
                .iter()
                .all(|c| c.info().is_some_and(|i| i.rarity == CardRarity::Common))
        );
        let picks = [select.offered[5], select.offered[2]];
        run.apply(RunAction::SelectCard(5)).expect("a pick");
        run.apply(RunAction::SelectCard(2)).expect("a pick");
        assert_eq!(run.state.deck[10..], picks);
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn the_future_of_potions_trades_for_upgraded_cards() {
        let mut run = in_event(EventKind::TheFutureOfPotions, |s| {
            s.potions = vec![Some("BONE_BREW"), None, Some("FIRE_POTION")];
        });
        assert_eq!(page(&run).len(), 2);
        // The second option is the potion in slot 2: a Common, so an Attack or Skill.
        run.apply(RunAction::EventOption(1)).expect("POTION");
        assert_eq!(run.state.potions, [Some("BONE_BREW"), None, None]);
        let RunDecision::Rewards([Reward::Card(cards)]) = run.pending_decision() else {
            panic!("a card reward");
        };
        let info: Vec<_> = cards.cards.iter().filter_map(|c| c.info()).collect();
        assert_eq!(info.len(), 3);
        assert!(info.iter().all(|i| i.rarity == CardRarity::Common));
        assert!(info.iter().all(|i| i.card_type == info[0].card_type));
        assert_ne!(info[0].card_type, CardType::Power);
        assert!(cards.cards.iter().all(|c| c.upgrade_level == 1));
        run.apply(RunAction::SkipReward(0)).expect("skip");
        run.apply(RunAction::Proceed).expect("leave the screen");
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn the_future_of_potions_handles_event_potions() {
        // Glowwater and Ambergris are event potions outside the pools; both are Event rarity,
        // which the event trades as Rare. A Glowwater in the belt used to stop the run when the
        // event began (`UnsupportedPotion`), and a map node entering it looked stuck.
        let mut run = in_event(EventKind::TheFutureOfPotions, |s| {
            s.potions = vec![Some("GLOWWATER_POTION"), None, Some("AMBERGRIS")];
        });
        assert_eq!(page(&run).len(), 2);
        run.apply(RunAction::EventOption(0)).expect("POTION");
        let RunDecision::Rewards([Reward::Card(cards)]) = run.pending_decision() else {
            panic!("a card reward");
        };
        assert!(
            cards
                .cards
                .iter()
                .all(|c| c.info().is_some_and(|i| i.rarity == CardRarity::Rare))
        );
        assert_eq!(run.state.potions, [None, None, Some("AMBERGRIS")]);
    }

    #[test]
    fn the_future_of_potions_handles_a_token_potion() {
        // Petrified Toad puts a Potion-Shaped Rock in the belt at the start of a fight; the token
        // pool is outside every potion pool, but the event treats Token like Common.
        let mut run = in_event(EventKind::TheFutureOfPotions, |s| {
            s.potions = vec![Some("POTION_SHAPED_ROCK"), None, Some("FIRE_POTION")];
        });
        assert_eq!(page(&run).len(), 2);
        run.apply(RunAction::EventOption(0)).expect("POTION");
        let RunDecision::Rewards([Reward::Card(cards)]) = run.pending_decision() else {
            panic!("a card reward");
        };
        let info: Vec<_> = cards.cards.iter().filter_map(|c| c.info()).collect();
        assert_eq!(info.len(), 3);
        assert!(info.iter().all(|i| i.rarity == CardRarity::Common));
        assert!(info.iter().all(|i| i.card_type != CardType::Power));
        assert_eq!(run.state.potions, [None, None, Some("FIRE_POTION")]);
    }

    #[test]
    fn unrest_site_rests_to_full_with_a_curse() {
        let mut run = in_event(EventKind::UnrestSite, |s| s.hp = 20);
        run.apply(RunAction::EventOption(0)).expect("REST");
        assert_eq!(run.state.hp, 66);
        assert_eq!(run.state.deck.last().map(|c| c.id), Some("POOR_SLEEP"));
        assert!(!run.state.deck.last().is_some_and(|c| c.is_upgradable()));
    }

    /// Plays the fight with the first legal action each time (cards before ending the turn).
    fn win_fight(run: &mut Run) {
        for _ in 0..500 {
            let RunDecision::Combat(_) = run.pending_decision() else {
                return;
            };
            let action = run.legal_actions()[0].clone();
            run.apply(action).expect("a legal combat action");
        }
        panic!("the fight did not end");
    }

    #[test]
    fn an_event_fight_offers_room_and_extra_rewards_then_resumes_the_event() {
        let mut run = in_event(EventKind::TestEventCombat, |_| {});
        let floor = run.state.total_floor();
        let normals = run.state.act().normal_encounters_visited;
        run.apply(RunAction::EventOption(0)).expect("FIGHT");
        assert!(matches!(run.pending_decision(), RunDecision::Combat(_)));
        assert!(matches!(
            run.state.room,
            Room::Combat {
                room_type: RoomType::Monster,
                encounter: "SLIMES_WEAK"
            }
        ));
        win_fight(&mut run);
        let RunDecision::Rewards(rewards) = run.pending_decision() else {
            panic!("the fight's rewards");
        };
        // Gold 10..20, then the extra 7 (sorted with the gold), [potion], the card reward.
        assert_eq!(rewards[1], Reward::Gold(7));
        assert!(matches!(rewards.last(), Some(Reward::Card(_))));
        run.apply(RunAction::Proceed).expect("leave the rewards");
        // Back on the event's next page; no new floor and no counted encounter.
        assert_eq!(page(&run), ["TEST_EVENT_COMBAT.AFTER"]);
        assert!(matches!(run.state.room, Room::Event { .. }));
        assert_eq!(run.state.total_floor(), floor);
        assert_eq!(run.state.act().normal_encounters_visited, normals);
        run.apply(RunAction::EventOption(0)).expect("AFTER");
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn a_fight_without_rewards_resumes_its_event_at_once() {
        // `BattlewornDummyEventV1Encounter.ShouldGiveRewards` is false: no rewards screen, the
        // event's `Resume` runs straight after the fight. The first legal action never kills the
        // 75-HP dummy in time, so it escapes (`RanOutOfTime`) and the event ends with nothing.
        let mut run = in_event(EventKind::BattlewornDummy, |_| {});
        let potions = run.state.player_rng[crate::rng::PlayerRngType::Rewards].counter();
        run.apply(RunAction::EventOption(0)).expect("SETTING_1");
        assert!(matches!(
            run.state.room,
            Room::Combat {
                encounter: "BATTLEWORN_DUMMY_EVENT_V1_ENCOUNTER",
                ..
            }
        ));
        win_fight(&mut run);
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
        assert!(run.event().is_none());
        assert_eq!(
            run.state.player_rng[crate::rng::PlayerRngType::Rewards].counter(),
            potions
        );
    }

    #[test]
    fn tinker_time_adds_a_mad_science_of_the_chosen_type_and_rider() {
        let mut run = in_event(EventKind::TinkerTime, |_| {});
        run.apply(RunAction::EventOption(0))
            .expect("CHOOSE_CARD_TYPE");
        let types = page(&run);
        assert_eq!(types.len(), 2);
        run.apply(RunAction::EventOption(0)).expect("a card type");
        let chosen = match types[0].rsplit('.').next() {
            Some("ATTACK") => CardType::Attack,
            Some("SKILL") => CardType::Skill,
            _ => CardType::Power,
        };
        assert_eq!(page(&run).len(), 2);
        run.apply(RunAction::EventOption(1)).expect("a rider");
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
        let card = *run.state.deck.last().expect("the new card");
        assert_eq!(card.id, "MAD_SCIENCE");
        assert_eq!(card.card_type(), Some(chosen));
        let combat = card.card().expect("MadScience is ported");
        assert_eq!(combat.card_type(), chosen);
        assert_eq!(combat.gains_block(), chosen == CardType::Skill);
    }

    #[test]
    fn an_event_fight_without_resume_goes_to_the_map() {
        let mut run = in_event(EventKind::TestEventCombat, |_| {});
        run.apply(RunAction::EventOption(1)).expect("AMBUSH");
        win_fight(&mut run);
        assert!(matches!(run.pending_decision(), RunDecision::Rewards(_)));
        run.apply(RunAction::Proceed).expect("leave the rewards");
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
        assert!(run.event().is_none());
    }

    #[test]
    fn wellspring_bottles_a_uniform_potion() {
        let mut run = in_event(EventKind::Wellspring, |_| {});
        let before = run.state.player_rng[crate::rng::PlayerRngType::Rewards].counter();
        run.apply(RunAction::EventOption(0)).expect("BOTTLE");
        let RunDecision::Rewards([Reward::Potion(_)]) = run.pending_decision() else {
            panic!("a potion reward");
        };
        // One `NextItem`, no rarity roll.
        assert_eq!(
            run.state.player_rng[crate::rng::PlayerRngType::Rewards].counter(),
            before + 1
        );
    }

    #[test]
    fn wellspring_bathes_away_a_card_for_a_guilty() {
        let mut run = in_event(EventKind::Wellspring, |_| {});
        run.apply(RunAction::EventOption(1)).expect("BATHE");
        let RunDecision::CardSelect(select) = run.pending_decision() else {
            panic!("a removal");
        };
        assert_eq!((select.options.len(), select.count), (10, 1));
        run.apply(RunAction::SelectCard(9)).expect("Unleash");
        let ids: Vec<&str> = run.state.deck.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 10);
        assert!(!ids.contains(&"UNLEASH"));
        assert_eq!(ids.last(), Some(&"GUILTY"));
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn morphic_grove_transforms_two_picked_cards_on_the_event_rng() {
        let mut run = in_event(EventKind::MorphicGrove, |s| s.gold = 150);
        run.apply(RunAction::EventOption(0)).expect("GROUP");
        assert_eq!(run.state.gold, 0);
        // Pick Unleash (9), then the first Strike (0).
        run.apply(RunAction::SelectCard(9)).expect("a pick");
        run.apply(RunAction::SelectCard(0)).expect("a pick");
        let deck = &run.state.deck;
        assert_eq!(deck.len(), 10);
        assert_eq!(
            deck.iter().filter(|c| c.id == "STRIKE_NECROBINDER").count(),
            3
        );
        assert!(!deck.iter().any(|c| c.id == "UNLEASH"));
        // The two replacements joined the end, from the pool's Common to Rare cards.
        for card in &deck[8..] {
            let rarity = card.info().map(|i| i.rarity);
            assert!(matches!(
                rarity,
                Some(CardRarity::Common | CardRarity::Uncommon | CardRarity::Rare)
            ));
        }
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
        let mut loner = in_event(EventKind::MorphicGrove, |_| {});
        let max_hp = loner.state.max_hp;
        loner.apply(RunAction::EventOption(1)).expect("LONER");
        assert_eq!(loner.state.max_hp, max_hp + 5);
    }

    #[test]
    fn trash_heap_dives_in_for_a_relic_or_grabs_gold_and_a_card() {
        let mut dive = in_event(EventKind::TrashHeap, |_| {});
        dive.apply(RunAction::EventOption(0)).expect("DIVE_IN");
        assert_eq!(dive.state.hp, 66 - 8);
        assert_eq!(dive.state.relics.len(), 2);
        assert!(matches!(dive.pending_decision(), RunDecision::Map(_)));
        let mut grab = in_event(EventKind::TrashHeap, |_| {});
        let gold = grab.state.gold;
        grab.apply(RunAction::EventOption(1)).expect("GRAB");
        assert_eq!((grab.state.gold, grab.state.deck.len()), (gold + 100, 11));
        assert!(matches!(grab.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn endless_conveyor_charges_per_dish_and_serves_the_salad_every_fifth_grab() {
        let mut run = in_event(EventKind::EndlessConveyor, |s| {
            s.gold = 1000;
            s.hp = 40;
        });
        let mut grabs = 0;
        while let RunDecision::Event(room) = run.pending_decision() {
            let option = &room.options[0];
            let key = option
                .text_key
                .rsplit('.')
                .next()
                .expect("a dish")
                .to_owned();
            if option.action.is_none() {
                break;
            }
            if grabs == 4 {
                assert_eq!(key, "SEAPUNK_SALAD");
            }
            grabs += 1;
            let gold = run.state.gold;
            run.apply(RunAction::EventOption(0)).expect("a dish");
            loop {
                match run.pending_decision() {
                    RunDecision::CardSelect(_) => run.apply(RunAction::SelectCard(0)),
                    RunDecision::Rewards(_) => run.apply(RunAction::ClaimReward(0)),
                    _ => break,
                }
                .expect("a prompt");
            }
            let paid = if key == "GOLDEN_FYSH" { -75 } else { 40 };
            assert_eq!(run.state.gold, gold - paid, "{key}");
            if grabs == 12 {
                break;
            }
        }
        assert_eq!(grabs, 12);
        run.apply(RunAction::EventOption(1)).expect("LEAVE");
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn dense_vegetation_rests_with_a_mimicked_heal_then_fights_four_wrigglers() {
        let mut run = in_event(EventKind::DenseVegetation, |s| s.hp = 40);
        assert_eq!(
            page(&run),
            [
                "DENSE_VEGETATION.pages.INITIAL.options.TRUDGE_ON",
                "DENSE_VEGETATION.pages.INITIAL.options.REST"
            ]
        );
        run.apply(RunAction::EventOption(1)).expect("REST");
        // `66 × 0.3m` = 19.8, `(int)(40 + 19.8)`.
        assert_eq!(run.state.hp, 59);
        assert_eq!(page(&run), ["DENSE_VEGETATION.pages.REST.options.FIGHT"]);
        let floor = run.state.total_floor();
        run.apply(RunAction::EventOption(0)).expect("FIGHT");
        assert!(matches!(
            run.state.room,
            Room::Combat {
                room_type: RoomType::Monster,
                encounter: "DENSE_VEGETATION_EVENT_ENCOUNTER"
            }
        ));
        assert_eq!(run.state.total_floor(), floor);
        let combat = run.combat().expect("the fight");
        let slots: Vec<_> = combat
            .state()
            .enemies
            .iter()
            .map(|&e| combat.state().creature(e).slot)
            .collect();
        assert_eq!(
            slots,
            [
                Some("wriggler1"),
                Some("wriggler2"),
                Some("wriggler3"),
                Some("wriggler4")
            ]
        );
    }

    #[test]
    fn dense_vegetation_trudges_on_for_gold_at_8_hp() {
        let mut run = in_event(EventKind::DenseVegetation, |s| s.gold = 0);
        run.apply(RunAction::EventOption(0)).expect("TRUDGE_ON");
        assert_eq!(run.state.hp, 66 - 8);
        assert!((61..100).contains(&run.state.gold));
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn whispering_hollow_pays_for_two_potions() {
        let mut run = in_event(EventKind::WhisperingHollow, |s| s.gold = 99);
        run.apply(RunAction::EventOption(0)).expect("GOLD");
        assert!((26..=44).contains(&(99 - run.state.gold)));
        let RunDecision::Rewards([Reward::Potion(_), Reward::Potion(_)]) = run.pending_decision()
        else {
            panic!("two potion rewards");
        };
    }

    #[test]
    fn whispering_hollow_hugs_a_transform_for_9_damage() {
        let mut run = in_event(EventKind::WhisperingHollow, |_| {});
        run.apply(RunAction::EventOption(1)).expect("HUG");
        run.apply(RunAction::SelectCard(9)).expect("Unleash");
        assert_eq!(run.state.hp, 66 - 9);
        assert!(!run.state.deck.iter().any(|c| c.id == "UNLEASH"));
        assert_eq!(run.state.deck.len(), 10);
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn luminous_choir_locks_the_tribute_below_its_rolled_gold() {
        let rich = in_event(EventKind::LuminousChoir, |s| s.gold = 149);
        assert_eq!(
            rich.event().map(|e| e.options[1].action.is_some()),
            Some(true)
        );
        let poor = in_event(EventKind::LuminousChoir, |s| s.gold = 99);
        assert_eq!(
            poor.event().map(|e| e.options[1].text_key.as_str()),
            Some("LUMINOUS_CHOIR.pages.INITIAL.options.OFFER_TRIBUTE_LOCKED")
        );
    }

    #[test]
    fn luminous_choir_removes_two_cards_for_a_spore_mind() {
        let mut run = in_event(EventKind::LuminousChoir, |_| {});
        run.apply(RunAction::EventOption(0))
            .expect("REACH_INTO_THE_FLESH");
        run.apply(RunAction::SelectCard(9)).expect("Unleash");
        run.apply(RunAction::SelectCard(8)).expect("Bodyguard");
        let ids: Vec<&str> = run.state.deck.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 9);
        assert_eq!(ids.last(), Some(&"SPORE_MIND"));
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn luminous_choir_trades_gold_for_the_next_relic() {
        let mut run = in_event(EventKind::LuminousChoir, |s| s.gold = 300);
        let relics = run.state.relics.len();
        // The next relic of the bag may not be ported; the option is then refused and the run
        // is unchanged.
        match run.apply(RunAction::EventOption(1)) {
            Ok(()) => {
                assert!((100..=149).contains(&(300 - run.state.gold)));
                assert_eq!(run.state.relics.len(), relics + 1);
            }
            Err(RunError::UnsupportedRelic(_)) => assert_eq!(run.state.gold, 300),
            Err(e) => panic!("OFFER_TRIBUTE: {e:?}"),
        }
    }

    #[test]
    fn aroma_of_chaos_upgrades_a_picked_card() {
        let mut run = in_event(EventKind::AromaOfChaos, |_| {});
        run.apply(RunAction::EventOption(1))
            .expect("MAINTAIN_CONTROL");
        run.apply(RunAction::SelectCard(0)).expect("a Strike");
        assert_eq!(run.state.deck[0].upgrade_level, 1);
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn wood_carvings_turns_a_basic_card_into_a_peck() {
        let mut run = in_event(EventKind::WoodCarvings, |_| {});
        assert_eq!(page(&run).len(), 3);
        run.apply(RunAction::EventOption(0)).expect("BIRD");
        run.apply(RunAction::SelectCard(0)).expect("a Strike");
        assert_eq!(run.state.deck.last().map(|c| c.id), Some("PECK"));
        assert_eq!(run.state.deck.len(), 10);
    }

    #[test]
    fn tea_master_sells_its_teas() {
        let mut run = in_event(EventKind::TeaMaster, |s| s.gold = 160);
        assert!(
            run.event()
                .is_some_and(|e| e.options.iter().all(|o| o.action.is_some()))
        );
        run.apply(RunAction::EventOption(1)).expect("EMBER_TEA");
        assert_eq!(run.state.gold, 10);
        assert_eq!(
            run.state.relics.last().map(|r| r.kind.id()),
            Some("EMBER_TEA")
        );
        let poor = in_event(EventKind::TeaMaster, |s| s.gold = 40);
        let keys = page(&poor);
        assert_eq!(keys[0], "TEA_MASTER.pages.INITIAL.options.BONE_TEA_LOCKED");
        assert_eq!(keys[1], "TEA_MASTER.pages.INITIAL.options.EMBER_TEA_LOCKED");
    }

    #[test]
    fn sapphire_seed_heals_and_upgrades_or_plants_a_sown_card() {
        let mut run = in_event(EventKind::SapphireSeed, |s| s.hp = 50);
        run.apply(RunAction::EventOption(0)).expect("EAT");
        assert_eq!(run.state.hp, 59);
        run.apply(RunAction::SelectCard(0)).expect("a Strike");
        assert_eq!(run.state.deck[0].upgrade_level, 1);

        let mut run = in_event(EventKind::SapphireSeed, |_| {});
        run.apply(RunAction::EventOption(1)).expect("PLANT");
        run.apply(RunAction::SelectCard(4)).expect("a Defend");
        let e = run.state.deck[4].enchantment.expect("enchanted");
        assert_eq!((e.kind, e.amount), (EnchantmentKind::Sown, 1));
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    #[test]
    fn self_help_book_locks_the_readings_the_deck_cannot_take() {
        let mut run = in_event(EventKind::SelfHelpBook, |_| {});
        // The starting deck has Attacks and Skills that gain block, but no Power.
        assert_eq!(
            page(&run),
            [
                "SELF_HELP_BOOK.pages.INITIAL.options.READ_THE_BACK",
                "SELF_HELP_BOOK.pages.INITIAL.options.READ_PASSAGE",
                "SELF_HELP_BOOK.pages.INITIAL.options.READ_ENTIRE_BOOK_LOCKED",
            ]
        );
        run.apply(RunAction::EventOption(1)).expect("READ_PASSAGE");
        // Only the Defends: Bodyguard is a Skill that does not gain block.
        let RunDecision::CardSelect(select) = run.pending_decision() else {
            panic!("a card selection");
        };
        assert_eq!(select.options, [4, 5, 6, 7]);
        run.apply(RunAction::SelectCard(0)).expect("a Defend");
        let e = run.state.deck[4].enchantment.expect("enchanted");
        assert_eq!((e.kind, e.amount), (EnchantmentKind::Nimble, 2));
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    }

    /// The `IsAllowed` overrides of the Underdocks events (`Models.Events/*.cs`).
    #[test]
    fn underdocks_event_predicates() {
        let mut run = t6s0();
        let s = &mut run.state;
        for (id, gold) in [("ENDLESS_CONVEYOR", 120), ("WATERLOGGED_SCRIPTORIUM", 55)] {
            s.gold = gold - 1;
            assert!(!is_allowed(id, s));
            s.gold = gold;
            assert!(is_allowed(id, s));
        }
        s.hp = 5;
        assert!(!is_allowed("TRASH_HEAP", s));
        s.hp = 6;
        assert!(is_allowed("TRASH_HEAP", s));
        // Neow is floor 1.
        assert_eq!(s.total_floor(), 1);
        assert!(!is_allowed("PUNCH_OFF", s));
        // The starter deck has Basic Strikes and Defends; enchanted or gone, Spiral fits nothing.
        assert!(is_allowed("SPIRALING_WHIRLPOOL", s));
        for card in &mut s.deck {
            card.enchantment = Some(Enchantment::new(EnchantmentKind::Nimble, 1));
        }
        assert!(!is_allowed("SPIRALING_WHIRLPOOL", s));
        assert!(is_allowed("SUNKEN_TREASURY", s));
    }
}
