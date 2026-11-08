//! The phone browser client's engine side: a run, the client state `sts2_view::ui::Ui` keeps and
//! every screen `sts2-view` builds, as JSON behind a few calls.
//!
//! The fight is `Ui::screen`; the rooms (Neow, events, the map, rewards, rest, treasure, the
//! shop) are `Ui::room_screen`; the run's end is `rooms::end_screen`; the run menu and its
//! lists are `Ui::sheet_screen`, sent beside the screen they cover. The game's text is handed
//! in by the page ([`load_text`]); without it names come from the engine.

use sts2_core::combat::{CardId, CreatureId, Status};
use sts2_core::content::cards::{CardKind, Keywords};
use sts2_core::rng::Rng;
use sts2_core::run::{
    Act1, Modifier, Run, RunAction, RunCard, RunDecision, SelectPurpose, UnlockState,
};
use sts2_core::{Action, Character};
use sts2_view::combat::POTION_KEYS;
use sts2_view::describe::Describer;
use sts2_view::model::{Block, ButtonState, Input, Key, Screen};
use sts2_view::rooms::{self, EndAction, Visit};
use sts2_view::sheets::{Context, Entry, Kind};
use sts2_view::text::{self, Localization, Role};
use sts2_view::ui::{Effect, Mode, Ui, ids};
use sts2_view::{names, visits};
use wasm_bindgen::prelude::wasm_bindgen;

mod taps;
pub use taps::Taps;
mod choices;
pub mod fx;
pub mod library;

use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    /// The game's text the page loaded, shared by every run it starts.
    static TEXT: RefCell<Rc<Localization>> = RefCell::new(Rc::new(Localization::empty()));
}

/// What the page should do once the screen is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    /// Back to the start screen (the run's save stays unless the run is over).
    Select,
    /// A new run of the same character on a fresh seed.
    NewSeed,
    /// The page's settings.
    Settings,
    /// The page's help.
    Help,
}

impl Leave {
    const fn word(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::NewSeed => "new-seed",
            Self::Settings => "settings",
            Self::Help => "help",
        }
    }
}

/// One run in progress and the client's half-made choices.
#[derive(Clone, Debug)]
pub struct Game {
    character: usize,
    seed: String,
    ascension: u8,
    /// The custom run's modifiers (empty for an ordinary run).
    modifiers: Vec<Modifier>,
    run: Run,
    ui: Ui,
    describer: Describer,
    /// The rooms entered so far, for the history and the end screen.
    visits: Vec<Visit>,
    /// What was chosen on each floor (GD26): the act, the floor and the choice ([`choices::of`]).
    choices: Vec<(usize, usize, serde_json::Value)>,
    /// Every action applied, as its index in [`Game::recordable_actions`] of the state it was
    /// applied to: with the seed, all a save needs.
    actions: Vec<u32>,
    /// The belt slot whose throw-away row of the menu asks for its second tap.
    tossing: Option<usize>,
    status: Option<String>,
    leave: Option<Leave>,
    /// A practice run: the last card or potion of the turn can be taken back ([`Game::undo`]).
    practice: bool,
    /// A Daily run: kept in the save and the history so progress can tell it from Standard.
    daily: bool,
    undo: fx::Undo,
    /// How the enemies' turn went, worded once for the page after End Turn.
    turn_note: Option<String>,
    /// The fight's events the last input caused ([`fx::events`]).
    events: Vec<serde_json::Value>,
    /// The deck cards the last input transformed, each `{from, to}` as card faces, for the
    /// page's reveal (`NCardTransformVfx`).
    reveal: Vec<serde_json::Value>,
    /// The gold the chest gave on entering a treasure room (`DoTreasureRoomRewards` gains it at
    /// once and bursts it as particles); kept while the room is open.
    chest_gold: Option<i32>,
    /// A reward screen left empty leaves by itself ([`sts2_view::ui::follow_up`]); off, it waits
    /// for its Proceed, as the game's.
    auto_proceed: bool,
}

impl Game {
    /// A new run, showing the text [`load_text`] loaded.
    ///
    /// # Errors
    /// The engine's refusal, in words (an ascension above 10).
    pub fn new(
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &[Modifier],
    ) -> Result<Self, String> {
        let chosen = Character::ALL.get(character).copied().unwrap_or_default();
        let run = Run::new_with_modifiers(
            chosen,
            seed,
            ascension,
            UnlockState::all(),
            Act1::Random,
            modifiers,
        )
        .map_err(|e| format!("{e:?}"))?
        .with_events();
        let mut describer = Describer::new(TEXT.with_borrow(Rc::clone));
        describer.refresh(&run);
        let mut game = Self {
            character,
            seed: seed.to_owned(),
            ascension,
            modifiers: modifiers.to_vec(),
            run,
            ui: Ui::default(),
            describer,
            visits: Vec::new(),
            choices: Vec::new(),
            actions: Vec::new(),
            tossing: None,
            status: None,
            leave: None,
            practice: false,
            daily: false,
            undo: fx::Undo::default(),
            turn_note: None,
            auto_proceed: true,
            events: Vec::new(),
            reveal: Vec::new(),
            chest_gold: None,
        };
        visits::note(&mut game.visits, &game.run, &game.describer);
        Ok(game)
    }

    /// The run on `seed` played on through `actions` (as [`Game::actions`] lists them).
    ///
    /// # Errors
    /// If the run cannot start or an action is not there to take where it is reached.
    pub fn replay(
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &[Modifier],
        actions: &[u32],
    ) -> Result<Self, String> {
        let mut game = Self::new(character, seed, ascension, modifiers)?;
        game.advance(actions)?;
        // A reload shows the run where it stands, not the transforms on the way there.
        game.reveal.clear();
        Ok(game)
    }

    /// Applies `actions` (numbered as [`Game::actions`] lists them) one by one.
    ///
    /// # Errors
    /// If an action is not there to take where it is reached.
    pub fn advance(&mut self, actions: &[u32]) -> Result<(), String> {
        for &index in actions {
            let step = self.actions.len() + 1;
            let action = self
                .recordable_actions()
                .into_iter()
                .nth(index as usize)
                .ok_or_else(|| format!("the save does not replay at action {step}"))?;
            self.apply_one(action);
        }
        self.status = None;
        Ok(())
    }

    /// The run.
    #[must_use]
    pub const fn run(&self) -> &Run {
        &self.run
    }

    /// The active modifiers as `{key, title, description, good}` rows, for the screens that
    /// list them.
    fn modifier_rows(&self) -> Vec<serde_json::Value> {
        self.modifiers
            .iter()
            .map(|&m| modifier_row(&self.describer, m))
            .collect()
    }

    /// The run's character, seed and ascension.
    #[must_use]
    pub fn start(&self) -> (usize, &str, u8) {
        (self.character, &self.seed, self.ascension)
    }

    /// The run's modifiers.
    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }

    /// Whether an emptied reward screen leaves by itself.
    pub const fn set_auto_proceed(&mut self, on: bool) {
        self.auto_proceed = on;
    }

    /// Makes this a practice run, or a strict one. Practice undoes nothing already played.
    pub fn set_practice(&mut self, practice: bool) {
        self.practice = practice;
        self.undo.clear();
    }

    /// Marks this as a Daily run.
    pub const fn set_daily(&mut self, daily: bool) {
        self.daily = daily;
    }

    /// Takes back the last card or potion played this turn, when nothing since has shown the
    /// player anything new; returns whether it did. Strict runs never do.
    pub fn undo(&mut self) -> bool {
        self.status = None;
        if !self.practice || !self.in_fight() || self.ui.sheet().is_some() {
            return false;
        }
        let Some((run, actions, visits)) = self.undo.pop() else {
            return false;
        };
        self.run = run;
        self.actions.truncate(actions);
        self.visits.truncate(visits);
        self.describer.refresh(&self.run);
        self.ui.cancel();
        self.ui.after_apply(&self.run);
        true
    }

    /// The actions applied so far.
    #[must_use]
    pub fn actions(&self) -> &[u32] {
        &self.actions
    }

    /// What an action can be, in the order the action list numbers them.
    #[must_use]
    pub fn recordable_actions(&self) -> Vec<RunAction> {
        let mut actions = self.run.legal_actions();
        actions.extend(self.run.discard_actions());
        actions.extend(self.run.unpick_actions());
        actions
    }

    /// Whether the run is over (died, won or stopped on content that is not ported).
    #[must_use]
    pub fn is_over(&self) -> bool {
        matches!(
            self.run.pending_decision(),
            RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
        )
    }

    fn in_fight(&self) -> bool {
        self.run.combat().is_some() && !self.is_over()
    }

    fn context(&self) -> Context<'_> {
        Context {
            run: &self.run,
            describer: &self.describer,
            visits: &self.visits,
            tossing: self.tossing.and_then(|slot| {
                let id = self.run.potions().get(slot).copied().flatten()?;
                Some((slot, format!("Throw away {}? Tap again", names::potion(id))))
            }),
        }
    }

    /// The screen under any sheet.
    #[must_use]
    pub fn screen(&self) -> Screen {
        let mut screen = if self.is_over() {
            rooms::end_screen(&self.run, &self.describer, &self.seed, &self.visits)
        } else if self.in_fight() {
            self.ui
                .screen(&self.run, &self.describer)
                .map(|s| self.unfocused(s))
        } else {
            self.ui.room_screen(&self.run, &self.describer, 0)
        }
        .unwrap_or_default();
        if screen.top.is_none() {
            screen.top = Some(self.ui.top_bar(&self.run, &self.describer));
        }
        if let Some(status) = &self.status {
            screen.status = Some(text::inline(status, Role::Plain));
        }
        screen
    }

    /// The fight with no keyboard focus: a phone has none, and a focused card is drawn
    /// `Selected` whether or not it can be played. With nothing waiting for a target, each hand
    /// row says again whether the engine lets it be played.
    fn unfocused(&self, mut screen: Screen) -> Screen {
        let Some(combat) = self.run.combat() else {
            return screen;
        };
        if *self.ui.mode() != Mode::Idle || combat.status() == Status::Decision {
            return screen;
        }
        let hand = &combat.state().player.hand;
        let legal = combat.legal_actions();
        for block in &mut screen.blocks {
            let Block::List { rows, .. } = block else {
                continue;
            };
            for row in rows.iter_mut().filter(|r| r.state == ButtonState::Selected) {
                let card = (row.id.0 as usize).checked_sub(1).and_then(|i| hand.get(i));
                let playable = card.is_some_and(|card| {
                    legal
                        .iter()
                        .any(|a| matches!(a, Action::PlayCard { card: c, .. } if c == card))
                });
                row.state = if playable {
                    ButtonState::Enabled
                } else {
                    ButtonState::Dim("it cannot be played now".to_owned())
                };
            }
        }
        if let Some(primary) = screen.primary.as_mut()
            && primary.state == ButtonState::Selected
        {
            primary.state = ButtonState::Enabled;
        }
        screen
    }

    /// The sheet open over the screen, as its own screen.
    #[must_use]
    pub fn sheet(&self) -> Option<Screen> {
        self.ui.sheet_screen(&self.context())
    }

    /// Opens a sheet by its name (`menu`, `deck`, `relics`, `info`, `map`, `history`, `log`,
    /// `draw`, `discard`, `exhaust`), or closes it when it is the one open.
    pub fn open(&mut self, name: &str) {
        let kind = match name {
            "menu" => Kind::Menu,
            "deck" => Kind::Deck,
            "relics" => Kind::Relics,
            "info" => Kind::Info,
            "map" => Kind::Map,
            "history" => Kind::History,
            "log" => Kind::Log,
            "draw" => Kind::DrawPile,
            "discard" => Kind::DiscardPile,
            "exhaust" => Kind::ExhaustPile,
            _ => return,
        };
        self.status = None;
        self.tossing = None;
        let effects = self.ui.toggle_sheet(&self.run, kind);
        self.carry_out(effects);
    }

    /// Handles one input from the page.
    pub fn input(&mut self, input: &Input) {
        self.status = None;
        self.events.clear();
        self.reveal.clear();
        let effects = if self.ui.sheet().is_some() {
            self.ui.sheet_input(&self.run, &self.describer, input)
        } else if self.is_over() {
            self.ui.end_input(input)
        } else if self.in_fight() {
            self.ui.input(&self.run, &self.describer, input)
        } else {
            self.room_input(input)
        };
        if !matches!(effects.as_slice(), [Effect::Menu(Entry::Toss(_))]) {
            self.tossing = None;
        }
        self.carry_out(effects);
    }

    /// A room's input: a potion of the belt goes to the potion's state machine; the main
    /// button is a tap on the row it stands for.
    fn room_input(&mut self, input: &Input) -> Vec<Effect> {
        let potion_waits = matches!(self.ui.mode(), Mode::Potion(_) | Mode::Target(_));
        match *input {
            Input::Tap(_) if potion_waits => self.ui.input(&self.run, &self.describer, input),
            Input::Tap(id) if matches!(ids::decode(id), ids::Hit::Potion(_)) => {
                let ids::Hit::Potion(slot) = ids::decode(id) else {
                    return Vec::new();
                };
                let key = POTION_KEYS.get(slot).copied().unwrap_or('z');
                self.ui
                    .input(&self.run, &self.describer, &Input::Key(Key::Char(key)))
            }
            _ if potion_waits => self.ui.input(&self.run, &self.describer, input),
            Input::Primary => match self.screen().primary {
                Some(primary) => {
                    self.ui
                        .room_input(&self.run, &self.describer, 0, &Input::Tap(primary.id))
                }
                None => Vec::new(),
            },
            _ => self.ui.room_input(&self.run, &self.describer, 0, input),
        }
    }

    fn carry_out(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Apply(action) => self.act(action),
                Effect::Error(message) => self.status = Some(message),
                Effect::End(action) => self.end(action),
                Effect::Menu(entry) => self.menu(entry),
                Effect::Pass => {}
            }
        }
    }

    fn end(&mut self, action: EndAction) {
        match action {
            EndAction::Retry => {
                if let Ok(game) =
                    Self::new(self.character, &self.seed, self.ascension, &self.modifiers)
                {
                    let daily = self.daily;
                    *self = game;
                    self.daily = daily;
                }
            }
            EndAction::NewSeed => self.leave = Some(Leave::NewSeed),
            EndAction::Deck => self.open("deck"),
            EndAction::Relics => self.open("relics"),
            EndAction::Visited => self.open("history"),
            EndAction::Characters | EndAction::Quit => self.leave = Some(Leave::Select),
        }
    }

    fn menu(&mut self, entry: Entry) {
        match entry {
            Entry::Sheet(_) => {}
            Entry::Help => self.leave = Some(Leave::Help),
            Entry::Settings => self.leave = Some(Leave::Settings),
            Entry::Quit => {
                self.ui.close_sheet();
                self.leave = Some(Leave::Select);
            }
            Entry::Toss(slot) if self.tossing == Some(slot) => {
                self.tossing = None;
                let action = RunAction::DiscardPotion(slot);
                if self.run.discard_actions().contains(&action) {
                    self.act(action);
                }
            }
            Entry::Toss(slot) => self.tossing = Some(slot),
        }
    }

    /// Applies `action` (one of the run's legal actions), then what follows on its own (a reward
    /// screen left empty leaves); each is recorded, so a replay applies the list as it is
    /// ([`Self::apply_one`]).
    pub fn act(&mut self, action: RunAction) {
        self.apply_one(action);
        while let Some(next) = sts2_view::ui::follow_up(&self.run).filter(|_| self.auto_proceed) {
            self.apply_one(next);
        }
    }

    /// Applies `action` alone, which comes from the engine's legal actions.
    fn apply_one(&mut self, action: RunAction) {
        let index = self.recordable_actions().iter().position(|a| *a == action);
        let play = matches!(
            action,
            RunAction::Combat(Action::PlayCard { .. } | Action::UsePotion { .. })
        );
        let ends = matches!(action, RunAction::Combat(Action::EndTurn));
        let poisoned = ends && self.run.combat().is_some_and(fx::poisoned);
        let noted = self.practice && play && self.in_fight();
        if noted {
            self.undo
                .push(&self.run, self.actions.len(), self.visits.len());
        }
        let combat_action = matches!(action, RunAction::Combat(_));
        let choice = (!combat_action)
            .then(|| choices::of(&self.run, &self.describer, &action))
            .flatten();
        let picked = choice.is_some();
        if let (Some(choice), Some(v)) = (choice, self.visits.last()) {
            self.choices.push((v.act, v.floor, choice));
        }
        let before = (!combat_action
            || matches!(action, RunAction::Combat(Action::UsePotion { .. })))
        .then(|| choices::Before::of(&self.run, &action));
        let floor = self.visits.last().map(|v| (v.act, v.floor));
        self.ui.cancel();
        self.ui.before_apply(&self.run, &action);
        let seen = self.run.combat().map_or(0, |c| c.events().len());
        let was_treasure = matches!(self.run.pending_decision(), RunDecision::Treasure(_));
        let orbs = self.run.combat().map(fx::orbs_before).unwrap_or_default();
        let blades = self.run.combat().map(fx::blades).unwrap_or_default();
        let piles = self.run.combat().map(fx::piles_before).unwrap_or_default();
        match self.run.apply(action) {
            Ok(()) => {
                if let Some(combat) = self.run.combat().or_else(|| self.run.ended_combat()) {
                    let new = combat.events().get(seen..).unwrap_or_default();
                    let mut events = fx::events(combat, new);
                    // A fight starts from a room, with no orbs before it.
                    fx::orb_events(&mut events, &orbs, &combat.state().player.orbs, ends);
                    fx::forge_event(&mut events, &blades, &fx::blades(combat));
                    fx::exhaust_events(&mut events, &piles, combat);
                    self.events.extend(events);
                }
                self.after_play(ends, poisoned);
                self.actions.push(
                    index
                        .and_then(|i| u32::try_from(i).ok())
                        .unwrap_or(u32::MAX),
                );
                if let (Some(before), Some((act, floor))) = (before.as_ref(), floor) {
                    let changes = before.changes(&self.run, combat_action);
                    // The pick of a card that was enchanted or transformed is that line already.
                    let reworded = changes
                        .iter()
                        .any(|c| matches!(c["kind"].as_str(), Some("enchant" | "transform")));
                    if picked && reworded {
                        self.choices.pop();
                    }
                    self.choices
                        .extend(changes.into_iter().map(|c| (act, floor, c)));
                }
                self.describer.refresh(&self.run);
                if let Some(before) = before.as_ref().filter(|_| !combat_action) {
                    self.note_reveals(before, was_treasure);
                }
                visits::note(&mut self.visits, &self.run, &self.describer);
                self.ui.after_apply(&self.run);
            }
            Err(e) => {
                if noted {
                    let _ = self.undo.pop();
                }
                self.status = Some(names::run_error(&e));
            }
        }
    }

    /// Notes what an action just applied shows the page once: the cards it transformed, and the
    /// chest's gold on entering a treasure room.
    fn note_reveals(&mut self, before: &choices::Before, was_treasure: bool) {
        let state = self.run.state();
        for (from, to) in before.transforms(&state.deck) {
            self.reveal.push(serde_json::json!({
                "from": rooms::deck_face(&self.describer, from),
                "to": rooms::deck_face(&self.describer, to),
            }));
        }
        let treasure = matches!(self.run.pending_decision(), RunDecision::Treasure(_));
        if treasure && !was_treasure {
            self.chest_gold = Some(state.gold - before.gold()).filter(|g| *g > 0);
        } else if !treasure {
            self.chest_gold = None;
        }
    }

    /// Keeps the practice undo and the turn line in step with an action just applied.
    fn after_play(&mut self, ends: bool, poisoned: bool) {
        if ends || !self.in_fight() {
            self.undo.clear();
        } else if let (Some(before), Some(now)) = (self.undo.last_combat(), self.run.combat())
            && fx::revealed(before, now)
        {
            self.undo.clear();
        }
        if ends && let Some(combat) = self.run.combat().filter(|_| self.in_fight()) {
            self.turn_note = Some(fx::turn_note(combat, poisoned));
        }
    }

    /// The finished run for the page's history: the result, what killed the player, the deck,
    /// the relics and the rooms walked. `null` while the run goes on.
    fn summary(&self) -> serde_json::Value {
        let result = match self.run.pending_decision() {
            RunDecision::Victory => "win",
            RunDecision::GameOver => "loss",
            RunDecision::Stopped(_) => "stopped",
            _ => return serde_json::Value::Null,
        };
        let state = self.run.state();
        let top = sts2_view::topbar::build(&self.run, &self.describer, None);
        let killer = (result == "loss")
            .then(|| rooms::cause(&self.run, &self.visits))
            .flatten();
        serde_json::json!({
            "result": result,
            "killer": killer,
            // The fight that ended the run (its `ModelId.Entry`), for the end screen's line.
            "encounter": self.run.ended_combat().map(|c| c.state().encounter.id()),
            "act": state.act_index + 1,
            // GD23: the run's acts (their ids), for the run history's act titles.
            "acts": state.acts.iter().map(|a| a.act.id).collect::<Vec<_>>(),
            "floor": state.total_floor(),
            "hp": state.hp,
            "max_hp": state.max_hp,
            "gold": state.gold,
            "deck": state.deck.iter().map(|c| serde_json::json!({
                "name": names::run_card(*c),
                "art": c.id,
                "level": c.upgrade_level,
                // GD23: the card as the deck sheet shows it, for the run history's inspect.
                "face": serde_json::to_value(rooms::deck_face(&self.describer, *c)).unwrap_or_default(),
            })).collect::<Vec<_>>(),
            "relics": state.relics.iter().map(|r| names::relic(r.kind.id())).collect::<Vec<_>>(),
            // GD23: the relics and the potion belt as the top bar shows them (art, name, text),
            // for the run history.
            "relic_chips": top.relics,
            "potion_chips": top.potions,
            "path": self.visits.iter().map(|v| serde_json::json!({
                "act": v.act, "floor": v.floor, "what": v.what, "hp": v.hp_out, "max_hp": v.max_hp,
                "gold": v.gold_out, "hp_in": v.hp_in,
                // GD26: what was chosen on the floor.
                "choices": self.choices.iter().filter(|(a, f, _)| *a == v.act && *f == v.floor)
                    .map(|(_, _, c)| c).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }

    /// The screen, the sheet over it and what the page needs to save and route, as JSON.
    #[must_use]
    pub fn view(&mut self) -> String {
        let sheet_kind = self.ui.sheet().map(|k| format!("{k:?}"));
        let view = serde_json::json!({
            "screen": self.screen(),
            "sheet": self.sheet(),
            "sheet_kind": sheet_kind,
            "decision": decision_word(&self.run.pending_decision()),
            "picture": picture_id(&self.run.pending_decision()),
            "fight": self.in_fight(),
            "over": self.is_over(),
            "room": self.run.state().room_type().map(sts2_core::run::RoomType::name),
            // GD9: the fight's encounter (its own music and ambience, src/audio.gd).
            "encounter": self.run.combat().or_else(|| self.run.ended_combat()).map(|c| c.state().encounter.id()),
            "summary": self.summary(),
            "leave": self.leave.take().map(Leave::word),
            "practice": self.practice,
            "undo": self.practice && self.undo.available() && self.in_fight(),
            "turn_note": self.turn_note.take(),
            "reveal": std::mem::take(&mut self.reveal),
            "chest_gold": self.chest_gold,
            // The game's tip per enemy intent, by creature id.
            "intent_tips": self.run.combat().filter(|_| self.in_fight()).map(|c| {
                sts2_view::combat::intent_tips(c, &self.describer)
                    .into_iter()
                    .map(|(id, tips)| (id.to_string(), serde_json::json!(tips.into_iter().map(|(title, text)| serde_json::json!({"title": title, "text": text})).collect::<Vec<_>>())))
                    .collect::<serde_json::Map<_, _>>()
            }),
            // WEB65: a selection from the deck that its last pick ends: the page holds that pick
            // for a preview with Confirm and Cancel (`NDeckCardSelectScreen.PreviewSelection`).
            "select": deck_select(&self.run.pending_decision()),
            // The fight's round (End Turn's number; a replayed run has no turn events).
            "round": self.run.combat().filter(|_| self.in_fight()).map(|c| c.state().round),
            // The event just finished: its last page, for a client that shows it before the map.
            "finished_event": rooms::finished_event_page(&self.run, &self.describer)
                .map(|(title, lines)| serde_json::json!({"title": title, "lines": lines})),
            "events": self.events,
            "run": {
                "act": self.run.state().act().act.id,
                "boss": self.run.state().act().boss.id,
                "second_boss": self.run.state().act().second_boss.map(|b| b.id),
                "ascension": self.ascension,
                "modifiers": self.modifier_rows(),
                "deck": self.run.state().deck.len(),
            },
            "save": {
                "character": self.character,
                "seed": self.seed,
                "ascension": self.ascension,
                "modifiers": self.modifiers.iter().map(|m| m.key()).collect::<Vec<_>>(),
                "practice": self.practice,
                "daily": self.daily,
                "actions": self.actions,
                "floor": self.run.state().total_floor(),
                "hp": self.run.state().hp,
                "max_hp": self.run.state().max_hp,
                // GD27: the main menu's Continue panel.
                "gold": self.run.state().gold,
                "act": self.run.state().act().act.id,
            },
        });
        view.to_string()
    }
}

/// The art entry (`event/ID`) of the picture over the room: an event's or ancient's own, the
/// treasure room's chest.
const fn picture_id(decision: &RunDecision<'_>) -> Option<&'static str> {
    match decision {
        RunDecision::Event(room) => Some(room.kind.id()),
        RunDecision::Ancient(room) => Some(room.kind.id()),
        RunDecision::Treasure(_) => Some("CHEST"),
        _ => None,
    }
}

/// A card selection from the deck: how many cards it takes, how many are picked, and whether its
/// last pick ends it (the page then previews that pick before sending it). Offers of new cards
/// (`AddToDeck`, `ChooseACard`) are not from the deck and take a pick at once, as in the game.
fn deck_select(decision: &RunDecision<'_>) -> Option<serde_json::Value> {
    let RunDecision::CardSelect(select) = decision else {
        return None;
    };
    let offers = matches!(
        select.purpose,
        SelectPurpose::AddToDeck | SelectPurpose::AddToDeckUpTo | SelectPurpose::ChooseACard { .. }
    );
    Some(serde_json::json!({
        "count": select.count,
        "picked": select.picked.len(),
        "preview": !offers && !select.manual_confirm(),
    }))
}

/// The pending decision's kind, for the page's styling and the headless proof.
const fn decision_word(decision: &RunDecision<'_>) -> &'static str {
    match decision {
        RunDecision::Ancient(_) => "ancient",
        RunDecision::Event(_) => "event",
        RunDecision::Map(_) => "map",
        RunDecision::Combat(_) => "combat",
        RunDecision::Rewards(_) => "rewards",
        RunDecision::CardSelect(_) => "card-select",
        RunDecision::Bundles(_) => "bundles",
        RunDecision::RestSite(_) => "rest",
        RunDecision::Treasure(_) => "treasure",
        RunDecision::Shop(_) => "shop",
        RunDecision::Unsupported(_) => "unsupported",
        RunDecision::Stopped(_) => "stopped",
        RunDecision::GameOver => "defeat",
        RunDecision::Victory => "victory",
    }
}

/// Loads the game's text: `json` is an object of table name to that table's JSON (the
/// `text/<table>.json` files `cargo xtask art-import` extracts). Runs started afterwards show it.
#[wasm_bindgen]
#[must_use]
pub fn load_text(json: &str) -> usize {
    let Ok(serde_json::Value::Object(tables)) = serde_json::from_str(json) else {
        return 0;
    };
    let tables: Vec<(String, String)> = tables
        .into_iter()
        .map(|(name, table)| (name, table.to_string()))
        .collect();
    let pairs: Vec<(&str, &str)> = tables
        .iter()
        .map(|(name, table)| (name.as_str(), table.as_str()))
        .collect();
    TEXT.set(Rc::new(Localization::from_json(&pairs)));
    pairs.len()
}

/// The tables [`load_text`] reads, as a JSON array.
#[wasm_bindgen]
#[must_use]
pub fn text_tables() -> String {
    serde_json::to_string(&[text::TABLES.as_slice(), text::WEB_TABLES.as_slice()].concat())
        .unwrap_or_default()
}

/// A modifier list as JSON keys (`["DRAFT","CHARACTER_CARDS:Silent"]`); `""` is none, which is
/// what a save from before modifiers has.
///
/// # Errors
///
/// A message when the JSON is not a list of known modifier keys.
pub fn parse_modifiers(json: &str) -> Result<Vec<Modifier>, String> {
    if json.is_empty() {
        return Ok(Vec::new());
    }
    let keys: Vec<String> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    keys.iter()
        .map(|k| Modifier::from_key(k).ok_or_else(|| format!("unknown modifier {k}")))
        .collect()
}

/// One modifier for the Custom Run screen: its key, localized title and description, and colour.
fn modifier_row(describer: &Describer, modifier: Modifier) -> serde_json::Value {
    serde_json::json!({
        "key": modifier.key(),
        "title": describer.modifier_title(modifier),
        "description": describer.modifier_description(modifier),
        "good": modifier.is_good(),
        "excludes": Modifier::selectable()
            .into_iter()
            .filter(|&o| modifier.excludes(o))
            .map(Modifier::key)
            .collect::<Vec<_>>(),
    })
}

/// The custom run screen's list (`NCustomRunModifiersList`): every selectable modifier, good
/// ones first, as JSON rows ([`modifier_row`]).
#[wasm_bindgen]
#[must_use]
pub fn modifier_list() -> String {
    let describer = Describer::new(TEXT.with_borrow(Rc::clone));
    let rows: Vec<serde_json::Value> = Modifier::selectable()
        .into_iter()
        .map(|m| modifier_row(&describer, m))
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// The Randomize button's modifiers.
///
/// `ModifierModel.Pick2Good1Bad` on a chaotic `Rng`, here seeded by the page: two good ones and one bad one, none a `CharacterCards` of
/// `character` (the lobby's character), as a JSON array of keys.
#[wasm_bindgen]
#[must_use]
pub fn random_modifiers(seed: f64, character: usize) -> String {
    // The page's random number: the whole part is the seed.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a JS seed"
    )]
    let mut rng = Rng::new(seed as u64);
    let chosen = Character::ALL.get(character).copied().unwrap_or_default();
    let keys: Vec<String> = Modifier::pick_2_good_1_bad(&mut rng, &[chosen])
        .into_iter()
        .map(Modifier::key)
        .collect();
    serde_json::json!(keys).to_string()
}

/// The daily run for `date` (`dd_MM_yyyy`, UTC) as JSON `{character, ascension, seed, modifiers}`:
/// what the date decides (`NDailyRunScreen.SetupLobbyParams`); `character` indexes the heroes
/// as [`Web::new`] does.
#[wasm_bindgen]
#[must_use]
pub fn daily_run(date: &str) -> String {
    let run = Modifier::daily_run(date);
    let character = Character::ALL
        .iter()
        .position(|&c| c == run.character)
        .unwrap_or_default();
    serde_json::json!({
        "character": character,
        "ascension": run.ascension,
        "seed": run.seed,
        "modifiers": run.modifiers.into_iter().map(Modifier::key).collect::<Vec<_>>(),
    })
    .to_string()
}

/// The page's handle on a run.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Web {
    game: Game,
}

#[wasm_bindgen]
impl Web {
    /// Starts a run: `character` indexes Ironclad, Silent, Defect, Regent, Necrobinder.
    ///
    /// # Errors
    /// The engine's refusal.
    #[wasm_bindgen(constructor)]
    pub fn new(
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            game: Game::new(character, seed, ascension, &parse_modifiers(modifiers)?)?,
        })
    }

    /// A saved run played back: `actions` is the save's JSON array.
    ///
    /// # Errors
    /// The run cannot start or the save does not play back.
    pub fn replay(
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &str,
        actions: &str,
    ) -> Result<Self, String> {
        let actions: Vec<u32> = serde_json::from_str(actions).map_err(|e| e.to_string())?;
        Ok(Self {
            game: Game::replay(
                character,
                seed,
                ascension,
                &parse_modifiers(modifiers)?,
                &actions,
            )?,
        })
    }

    /// Marks the run as a Daily.
    #[allow(
        clippy::missing_const_for_fn,
        reason = "wasm_bindgen exports no const fn"
    )]
    pub fn set_daily(&mut self, daily: bool) {
        self.game.set_daily(daily);
    }

    /// Makes the run a practice run (undo allowed) or a strict one.
    pub fn set_practice(&mut self, practice: bool) {
        self.game.set_practice(practice);
    }

    /// Off: a reward screen left empty waits for its Proceed (the game's), instead of leaving.
    #[allow(
        clippy::missing_const_for_fn,
        reason = "wasm_bindgen exports no const fn"
    )]
    pub fn set_auto_proceed(&mut self, on: bool) {
        self.game.set_auto_proceed(on);
    }

    /// On: the shop shows its whole stock on one level (every item with its price, then Remove a
    /// card) instead of its sections; off (the default) keeps the two levels.
    #[allow(
        clippy::missing_const_for_fn,
        reason = "wasm_bindgen exports no const fn"
    )]
    pub fn set_flat_shop(&mut self, on: bool) {
        self.game.ui.set_flat_shop(on);
    }

    /// Takes back the last card or potion of this turn in a practice run, when nothing since
    /// has revealed anything; returns the new view either way.
    #[must_use]
    pub fn undo(&mut self) -> String {
        let _ = self.game.undo();
        self.game.view()
    }

    /// What playing the hand card `card` at enemy `target` (`-1`: none) would do, on a copy of
    /// the fight: damage, Block and HP deltas and the enemies' turn after it, as JSON; `null`
    /// when the engine cannot say.
    #[must_use]
    pub fn preview(&self, card: u32, target: i32) -> String {
        let target = u32::try_from(target).ok().map(CreatureId);
        self.game
            .run
            .combat()
            .and_then(|c| fx::preview(c, CardId(card), target))
            .map_or_else(|| "null".to_owned(), |v| v.to_string())
    }

    /// The hand card `card` as the view's `CardFace` JSON with its numbers against enemy `target`
    /// (`-1`: no target), as the game shows a card being aimed; `null` outside a fight.
    #[must_use]
    pub fn card_face(&self, card: u32, target: i32) -> String {
        let target = u32::try_from(target).ok().map(CreatureId);
        self.game.run.combat().map_or_else(
            || "null".to_owned(),
            |c| {
                let face =
                    sts2_view::combat::card_face_at(c, &self.game.describer, CardId(card), target);
                serde_json::to_string(&face).unwrap_or_else(|_| "null".to_owned())
            },
        )
    }

    /// The view as JSON (`screen`, `sheet`, `save`, ...).
    #[must_use]
    pub fn view(&mut self) -> String {
        self.game.view()
    }

    /// Applies an `Input` given as JSON; returns the new view.
    #[must_use]
    pub fn input(&mut self, json: &str) -> String {
        match serde_json::from_str::<Input>(json) {
            Ok(input) => self.game.input(&input),
            Err(e) => self.game.status = Some(format!("bad input: {e}")),
        }
        self.game.view()
    }

    /// The relics a treasure's or an ancient's rows offer, as JSON `[{id, relic}]` (`id` the
    /// row's button, `relic` its id for the page's picture and text); `[]` elsewhere.
    #[must_use]
    pub fn room_relics(&self) -> String {
        let game = &self.game;
        let decision = game.run.pending_decision();
        let mut out = Vec::new();
        let mut number = 0_u32;
        for item in game.ui.room_items(&game.run, &game.describer) {
            if !item.enabled {
                continue;
            }
            number += 1;
            let relic = match (&item.action, &decision) {
                (RunAction::AncientOption(i), RunDecision::Ancient(room)) => {
                    room.options.get(*i).and_then(|o| o.relic_id())
                }
                (RunAction::TakeRelic, RunDecision::Treasure(relic)) => *relic,
                _ => None,
            };
            if let Some(relic) = relic {
                out.push(serde_json::json!({ "id": number, "relic": relic }));
            }
        }
        serde_json::Value::Array(out).to_string()
    }

    /// The view an `Input` (JSON) would show, on a copy of the game: what tapping a card reward
    /// opens, drawn on the rewards screen before it is opened. `null` when the input is not
    /// understood or would take an action (a peek never plays).
    #[must_use]
    pub fn peek(&self, json: &str) -> String {
        let Ok(input) = serde_json::from_str::<Input>(json) else {
            return "null".to_owned();
        };
        let mut copy = self.game.clone();
        copy.input(&input);
        if copy.actions().len() != self.game.actions().len() || copy.leave.is_some() {
            return "null".to_owned();
        }
        copy.view()
    }

    /// A test hook (`?encounter=`/`?cards=`, never in the UI): while the run
    /// waits on the map, `{"encounter": id, "cards": [id, ...]}` makes the next map fight the given
    /// encounter (any kind), hands the deck the cards and `"potions": [id, ...]` fills the belt from slot 0. Changes the run outside
    /// its actions, so a save of it does not replay. Returns the refusal, or "" when done.
    #[must_use]
    pub fn debug(&mut self, json: &str) -> String {
        self.debug_loadout(json).err().unwrap_or_default()
    }

    /// Throws away the potion in belt slot `slot`, when the engine allows it; returns the new
    /// view.
    #[must_use]
    pub fn toss(&mut self, slot: usize) -> String {
        self.game.status = None;
        let action = RunAction::DiscardPotion(slot);
        if self.game.run.discard_actions().contains(&action) {
            self.game.act(action);
        }
        self.game.view()
    }

    /// The combat background's layer picks (`BackgroundAssets`, `NCombatRoom.
    /// GenerateBackgroundRngForCurrentPoint`): `counts` is the JSON list of how many variants each
    /// layer slot has, the background slots in order and the foreground last; the answer is the
    /// chosen index of each, drawn from `Rng(seed + row + col · 747)` at the current map point.
    #[must_use]
    pub fn background(&self, counts: &str) -> String {
        let counts: Vec<u32> = serde_json::from_str(counts).unwrap_or_default();
        let state = self.game.run.state();
        let point = state
            .coord()
            .map_or(0, |c| u64::try_from(c.row + c.col * 747).unwrap_or(0));
        // The run's seed from its string: in a fight the run's streams belong to the combat and
        // `state.rng` is a placeholder seeded 0.
        let seed = sts2_core::rng::xxh64(&sts2_core::rng::canonicalize_seed(&self.game.seed));
        let mut rng = Rng::new(seed.wrapping_add(point));
        let picks: Vec<i32> = counts
            .iter()
            .map(|&n| {
                let n = i32::try_from(n).unwrap_or(0);
                if n > 0 { rng.next_int_range(0, n) } else { 0 }
            })
            .collect();
        serde_json::to_string(&picks).unwrap_or_default()
    }

    /// Which of the act's `n` background tracks the run plays (`NRunMusicController.ResolveMusic`:
    /// `Rng(run seed, "bg_music").NextInt(0, n)`), the same for the whole run.
    #[must_use]
    pub fn bg_music(&self, n: i32) -> i32 {
        let seed = sts2_core::rng::xxh64(&sts2_core::rng::canonicalize_seed(&self.game.seed));
        if n > 0 {
            Rng::named(seed, "bg_music").next_int_range(0, n)
        } else {
            0
        }
    }

    /// The game's glossary for the lit words of a text (`["Doom","Block"]` as JSON): each term
    /// it explains once, as `[{title, text}]` JSON with `text` as rich spans.
    #[must_use]
    pub fn glossary(&self, words: &str) -> String {
        let words: Vec<String> = serde_json::from_str(words).unwrap_or_default();
        let mut markup = String::new();
        for w in &words {
            markup.push_str("[gold]");
            markup.push_str(w);
            markup.push_str("[/gold] ");
        }
        let terms: Vec<serde_json::Value> = self
            .game
            .describer
            .glossary(&markup, Keywords::NONE)
            .into_iter()
            .map(|t| serde_json::json!({ "title": t.title, "text": text::inline(&t.text, Role::Body) }))
            .collect();
        serde_json::Value::Array(terms).to_string()
    }

    /// A relic's or potion's text by its id (`kind` is `relic` or `potion`), as offered: rich
    /// lines as JSON, `[]` when there is none.
    #[must_use]
    pub fn describe(&self, kind: &str, id: &str) -> String {
        let d = &self.game.describer;
        let markup = match kind {
            "relic" => d.relic(id),
            "potion" => d.potion(id),
            _ => None,
        };
        let lines = markup.map_or_else(Vec::new, |m| text::lines(&m, Role::Plain));
        serde_json::to_string(&lines).unwrap_or_default()
    }

    /// A card as it is and one upgrade up, as `{now, up}` JSON of card faces (`CardFace`), for
    /// the page's upgrade preview. `id` is the face's `art` (the card's entry) and `level` the
    /// times it is upgraded; `null` when the card is unknown or cannot be upgraded further.
    #[must_use]
    pub fn upgrade_preview(&self, id: &str, level: u8) -> String {
        let Some(entry) = CardKind::from_id(id).map(CardKind::entry) else {
            return "null".to_owned();
        };
        let mut now = RunCard::new(entry);
        now.upgrade_level = level;
        if !now.is_upgradable() {
            return "null".to_owned();
        }
        let mut up = now;
        up.upgrade();
        let d = &self.game.describer;
        serde_json::json!({
            "now": rooms::card_face(d, now),
            "up": rooms::card_face(d, up),
        })
        .to_string()
    }

    /// How the page makes action `index` (numbered as the save's actions are) from the screen
    /// as it is now, as JSON: `{taps, label, aims, potion}` ([`Taps`]; no taps when none was
    /// found, the label then names the action).
    #[must_use]
    pub fn taps(&self, index: u32) -> String {
        let Some(action) = self
            .game
            .recordable_actions()
            .into_iter()
            .nth(index as usize)
        else {
            return serde_json::Value::Null.to_string();
        };
        self.game
            .taps_for(&action)
            .unwrap_or_else(|| Taps {
                label: self.game.action_words(&action),
                ..Taps::default()
            })
            .json()
            .to_string()
    }

    /// Opens (or closes) a sheet by name; returns the new view.
    #[must_use]
    pub fn open(&mut self, name: &str) -> String {
        self.game.open(name);
        self.game.view()
    }
}

impl Web {
    fn debug_loadout(&mut self, json: &str) -> Result<(), String> {
        use sts2_core::run::RunCard;
        use sts2_core::run::rooms::ALL_ACTS;
        let spec: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let state = self
            .game
            .run
            .loadout_state()
            .map_err(|e| format!("not on the map: {e:?}"))?;
        // GD27: `?act=3` makes the run's act the third, so its boss ends the run (the victory screen).
        if let Some(act) = spec["act"].as_u64() {
            let index = usize::try_from(act)
                .ok()
                .and_then(|a| a.checked_sub(1))
                .filter(|&i| i < state.acts.len())
                .ok_or_else(|| format!("no act {act}"))?;
            state.act_index = index;
        }
        // GD27: `?hp=N` sets the hero's current and maximum HP (a run that can outlast a boss).
        if let Some(hp) = spec["hp"].as_i64().and_then(|h| i32::try_from(h).ok()) {
            state.hp = hp;
            state.max_hp = hp;
        }
        // GD46: `?gold=N` sets the gold (the shop's buys).
        if let Some(gold) = spec["gold"].as_i64().and_then(|g| i32::try_from(g).ok()) {
            state.gold = gold;
        }
        if let Some(id) = spec["encounter"].as_str() {
            let info = ALL_ACTS
                .iter()
                .flat_map(|a| a.encounters.iter())
                .find(|e| e.id == id)
                .ok_or_else(|| format!("no encounter {id}"))?;
            // Whatever its kind, the next map fight (the room type follows the encounter).
            state.acts[state.act_index].normal_encounters = vec![info];
        }
        for card in spec["cards"].as_array().into_iter().flatten() {
            let id: &'static str = Box::leak(
                card.as_str()
                    .unwrap_or_default()
                    .to_owned()
                    .into_boxed_str(),
            );
            let card = RunCard::new(id);
            // GD41: any ported card (an Event card such as `BYRD_SWOOP` is in no character pool).
            if card.info().is_none() && card.kind().is_none() {
                return Err(format!("no card {id}"));
            }
            state.add_card(card);
        }
        // The belt, slot by slot (`potions`); more ids than slots are refused.
        if let Some(ids) = spec["potions"].as_array() {
            if ids.len() > state.potions.len() {
                return Err(format!(
                    "{} potions, {} slots",
                    ids.len(),
                    state.potions.len()
                ));
            }
            for (slot, id) in ids.iter().enumerate() {
                let id = id.as_str().unwrap_or_default();
                let kind = sts2_core::content::potions::PotionKind::ALL
                    .iter()
                    .find(|k| k.id() == id)
                    .ok_or_else(|| format!("no potion {id}"))?;
                state.potions[slot] = Some(kind.id());
            }
        }
        if let Some(room) = spec["room"].as_str() {
            self.debug_room(room)?;
        }
        Ok(())
    }

    /// GD28: `?room=shop|rest|treasure|event[:ID]` enters (`boss`: stands under the boss instead) a map point of that kind at once: the
    /// run is moved to a parent of one (or the start) and the point is chosen. `event:ID` makes the
    /// act's next event `ID` and turns the `?` odds into events only.
    fn debug_room(&mut self, room: &str) -> Result<(), String> {
        use sts2_core::run::map::{MapCoord, MapPointType};
        let (kind, event) = match room.split_once(':') {
            Some((k, id)) => (k, Some(id)),
            None => (room, None),
        };
        if kind == "boss" {
            // The run stands on a room right under the boss, so the boss is the next room.
            let state = self
                .game
                .run
                .loadout_state()
                .map_err(|e| format!("not on the map: {e:?}"))?;
            let boss = state.map.boss();
            let parent = boss.parents.first().ok_or("the boss has no parent")?;
            state.visited.clear();
            state.visited.push(parent);
            return Ok(());
        }
        let want = match kind {
            "shop" => MapPointType::Shop,
            "rest" => MapPointType::RestSite,
            "treasure" => MapPointType::Treasure,
            "event" => MapPointType::Unknown,
            _ => return Err(format!("no room kind {kind}")),
        };
        let state = self
            .game
            .run
            .loadout_state()
            .map_err(|e| format!("not on the map: {e:?}"))?;
        let point = state
            .map
            .points()
            .iter()
            .find(|p| p.kind == want)
            .ok_or_else(|| format!("no {kind} on this map"))?;
        let coord: MapCoord = point.coord;
        let parent = point.parents.first();
        state.visited.clear();
        state.visited.extend(parent);
        if let Some(id) = event {
            let id: &'static str = Box::leak(id.to_owned().into_boxed_str());
            state.acts[state.act_index].events = vec![id];
            state.unknown_odds.odds = state.unknown_odds.odds.map(|(r, _)| (r, 0.0));
        }
        self.game.act(RunAction::ChooseNode(coord));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_view::model::Button;

    fn view(web: &mut Web) -> serde_json::Value {
        serde_json::from_str(&web.view()).expect("json")
    }

    fn rows(screen: &Screen) -> Vec<&Button> {
        screen
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::List { rows, .. } => Some(rows),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// Taps the first enabled row (or the main button) until a fight is on.
    fn to_fight(web: &mut Web) {
        for _ in 0..60 {
            if web.game.in_fight() {
                return;
            }
            let screen = web.game.screen();
            let tap = rows(&screen)
                .into_iter()
                .find(|r| r.state == ButtonState::Enabled)
                .map_or(Input::Primary, |r| Input::Tap(r.id));
            let _ = web.input(&serde_json::to_string(&tap).expect("json"));
        }
        panic!("no fight reached");
    }

    #[test]
    fn a_run_plays_to_a_fight_through_json() {
        let mut web = Web::new(4, "web-test", 0, "").expect("run");
        to_fight(&mut web);
        let v = view(&mut web);
        assert_eq!(v["fight"], true);
        assert!(v["screen"]["primary"].is_object(), "End Turn");
    }

    #[test]
    fn a_save_replays_to_the_same_screen() {
        let mut web = Web::new(4, "web-save", 0, "").expect("run");
        to_fight(&mut web);
        let _ = web.input(r#"{"Tap":999}"#);
        let actions = serde_json::to_string(web.game.actions()).expect("json");
        let mut back = Web::replay(4, "web-save", 0, "", &actions).expect("replay");
        assert_eq!(view(&mut back)["screen"], view(&mut web)["screen"]);
    }

    #[test]
    fn an_upgrade_preview_has_both_faces() {
        let web = Web::new(0, "web-up", 0, "").expect("run");
        let v: serde_json::Value =
            serde_json::from_str(&web.upgrade_preview("STRIKE_IRONCLAD", 0)).expect("json");
        assert_eq!(v["now"]["name"], "Strike");
        assert_eq!(v["up"]["name"], "Strike+");
        assert_eq!(web.upgrade_preview("STRIKE_IRONCLAD", 1), "null");
        assert_eq!(web.upgrade_preview("NoSuchCard", 0), "null");
    }

    #[test]
    fn a_finished_run_has_a_summary() {
        let mut web = Web::new(0, "web-sum", 0, "").expect("run");
        assert!(view(&mut web)["summary"].is_null());
    }

    #[test]
    fn the_menu_opens_as_a_sheet_and_closes_with_back() {
        let mut web = Web::new(0, "web-menu", 0, "").expect("run");
        let v: serde_json::Value = serde_json::from_str(&web.open("deck")).expect("json");
        assert_eq!(v["sheet_kind"], "Deck");
        let v: serde_json::Value = serde_json::from_str(&web.input(r#""Back""#)).expect("json");
        assert!(v["sheet"].is_null());
    }

    /// Every certain preview of the opening hand matches what the engine then does.
    #[test]
    fn a_preview_matches_what_the_engine_applies() {
        let mut checked = 0;
        for (character, seed) in [
            (0, "fx-a"),
            (1, "fx-b"),
            (2, "fx-c"),
            (3, "fx-d"),
            (4, "fx-e"),
        ] {
            let mut web = Web::new(character, seed, 0, "").expect("run");
            to_fight(&mut web);
            let combat = web.game.run.combat().expect("fight").clone();
            let me = combat.state().player.creature;
            for action in combat.legal_actions() {
                let Action::PlayCard { card, target } = action else {
                    continue;
                };
                let target_id = target.map_or(-1, |t| i32::try_from(t.0).expect("id"));
                let preview: serde_json::Value =
                    serde_json::from_str(&web.preview(card.0, target_id)).expect("json");
                if preview.is_null() || preview["partial"] == true {
                    continue;
                }
                let mut played = combat.clone();
                played.apply(action).expect("legal");
                for e in preview["enemies"].as_array().expect("list") {
                    let id = CreatureId(u32::try_from(e["id"].as_u64().expect("id")).expect("id"));
                    let (b, a) = (combat.state().creature(id), played.state().creature(id));
                    assert_eq!(e["hp_loss"], b.hp - a.hp, "hp loss of {seed} {card:?}");
                    assert_eq!(e["damage"], b.hp - a.hp + (b.block - a.block).max(0));
                    assert_eq!(e["dies"], b.is_alive() && !a.is_alive());
                }
                let (b, a) = (combat.state().creature(me), played.state().creature(me));
                assert_eq!(preview["you"]["block_gain"], (a.block - b.block).max(0));
                assert_eq!(preview["you"]["hp_delta"], a.hp - b.hp);
                if let (Some(i), Some(real)) = (preview["incoming"].as_object(), played.incoming())
                {
                    assert_eq!(i["hp_loss"], real.hp_loss);
                    assert_eq!(i["hp_after"], real.hp_after);
                }
                checked += 1;
            }
        }
        assert!(checked >= 15, "only {checked} previews checked");
    }

    fn fight_with(practice: bool) -> Web {
        let mut web = Web::new(0, "fx-undo", 0, "").expect("run");
        web.set_practice(practice);
        to_fight(&mut web);
        web
    }

    /// A play that draws nothing and rolls nothing: an attack or a skill that only hits or blocks.
    fn quiet_play(web: &Web) -> Option<RunAction> {
        let combat = web.game.run.combat()?;
        combat.legal_actions().into_iter().find_map(|a| {
            let Action::PlayCard { card, target } = a else {
                return None;
            };
            let outcome = combat.play_outcome(card, target)?;
            let after = {
                let mut c = combat.clone();
                c.apply(a.clone()).ok()?;
                c
            };
            (!outcome.wins && !fx::revealed(combat, &after)).then_some(RunAction::Combat(a))
        })
    }

    #[test]
    fn practice_undo_takes_back_a_quiet_play_and_the_save_replays() {
        let mut web = fight_with(true);
        let start = view(&mut web)["screen"].clone();
        let at = web.game.actions().len();
        let play = quiet_play(&web).expect("a quiet play");
        web.game.act(play);
        assert_eq!(view(&mut web)["undo"], true);
        assert_ne!(view(&mut web)["screen"], start);
        let v: serde_json::Value = serde_json::from_str(&web.undo()).expect("json");
        assert_eq!(v["screen"], start);
        assert_eq!(web.game.actions().len(), at);
        assert_eq!(v["undo"], false);
        assert_eq!(v["save"]["practice"], true);
    }

    #[test]
    fn strict_runs_and_end_of_turn_do_not_undo() {
        let mut strict = fight_with(false);
        let play = quiet_play(&strict).expect("a quiet play");
        strict.game.act(play);
        let v = view(&mut strict);
        assert_eq!(v["undo"], false);
        assert_eq!(v["save"]["practice"], false);
        let before = v["screen"].clone();
        assert_eq!(view_after_undo(&mut strict), before);

        let mut web = fight_with(true);
        let play = quiet_play(&web).expect("a quiet play");
        web.game.act(play);
        web.game.act(RunAction::Combat(Action::EndTurn));
        let v = view(&mut web);
        assert_eq!(v["undo"], false);
        assert!(v["turn_note"].is_string() || !web.game.in_fight());
    }

    fn view_after_undo(web: &mut Web) -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&web.undo()).expect("json")["screen"].clone()
    }

    #[test]
    fn a_play_that_draws_cannot_be_undone() {
        for seed in 0..30 {
            let mut web = Web::new(4, &format!("fx-draw-{seed}"), 0, "").expect("run");
            web.set_practice(true);
            to_fight(&mut web);
            let combat = web.game.run.combat().expect("fight");
            let drawing = combat.legal_actions().into_iter().find(|a| {
                let mut c = combat.clone();
                c.apply(a.clone()).is_ok()
                    && matches!(a, Action::PlayCard { .. })
                    && fx::revealed(combat, &c)
                    && c.status() != Status::Decision
            });
            if let Some(action) = drawing {
                web.game.act(RunAction::Combat(action));
                assert_eq!(view(&mut web)["undo"], false);
                return;
            }
        }
        panic!("no revealing play found");
    }
}
