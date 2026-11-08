//! UI state and input handling. No drawing here and no game rules: the engine's
//! `legal_actions` decides what is allowed.

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use sts2_core::Character;

use ratatui::crossterm::event::KeyCode;
use sts2_core::combat::{CardId, CreatureId, IncomingPreview};
use sts2_core::content::relics::RelicData;
use sts2_core::run::{
    Act1, MapCoord, RoomType, Run, RunAction, RunDecision, RunError, SelectPurpose, UnlockState,
};
use sts2_core::{Action, Combat, Status};
use sts2_view::model::{ButtonId, ButtonState, Input as ViewInput, Key as ViewKey};
use sts2_view::sheets::{self, Entry, Kind};
use sts2_view::ui::{self as view_ui, Effect, Item, Ui};

use crate::describe::Describer;
use crate::hit::{Hits, Pane, Target, UiAction};
use crate::keys::{self, Command, Layer};
use crate::menu::{self, MenuItem};
use crate::names;
use crate::text::Localization;

/// What the next key press means in a fight (the model's state machine).
pub use sts2_view::ui::{Mode as Input, Pending};

pub use sts2_view::rooms::{EndAction, Visit};

/// Where the player asked to go from the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leave {
    Quit,
    /// The character select (from the end screen).
    Select,
}

/// A menu entry that cannot be undone (Leave with rewards left, Skip, Buy, Remove), pressed
/// once: it shows its question in place and waits for the same key (or Enter, or a click on it
/// again); any other key puts it back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Confirm {
    action: RunAction,
    /// The question, without its `?`.
    what: String,
    /// The entry's number among the enabled entries (0 for a row outside them: a potion's).
    entry: usize,
    /// The press that takes it when it comes again (the entry's number, the potion's row).
    again: UiAction,
    /// The key that pressed it, if a key did (a digit, `s`).
    key: Option<KeyCode>,
}

/// Why a saved action list could not be played back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// The run itself cannot be created.
    Start(String),
    /// Action `step` (0-based) is not a legal action of the state it reached.
    Step(usize),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(e) => write!(f, "the run cannot start: {e}"),
            Self::Step(n) => write!(f, "action {} no longer applies", n + 1),
        }
    }
}

/// A full-screen list over the current screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Overlay {
    /// The whole deck.
    Deck,
    /// The draw pile, shown sorted: the game hides its order.
    DrawPile,
    DiscardPile,
    ExhaustPile,
    Relics,
    /// The potions and powers in play, with their text.
    Info,
    /// The act's map, enlarged.
    Map,
    /// The rooms visited so far.
    History,
    /// The fight's log: the plays and the enemy turns, the newest at the bottom.
    Log,
    /// One thing in full (`i` then a number).
    Inspect(Inspect),
    /// The menu behind the top bar's `≡`: the lists above, the help, the settings, quit.
    Menu,
}

/// The list the keyboard focus moves in, so a focus kept from one frame still means something
/// in the next: the same screen keeps it, another screen starts afresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    /// The choices of a room (the floor and the kind of decision, so a new room starts afresh).
    Menu(usize, u8),
    End,
    /// The `≡` menu.
    OverlayMenu,
}

/// Where the keyboard focus rests: what it is on, and its place in the list it was on, so
/// when that thing goes (a card is played) the focus stays where it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Focus {
    screen: Screen,
    target: Target,
    pos: usize,
}

/// The `≡` menu's entries: the key that opens each from any screen, the label and what the
/// entry does. `fight` adds the fight's log and piles.
#[must_use]
pub fn menu_entries(fight: bool) -> Vec<(char, &'static str, UiAction)> {
    sheets::menu_entries(fight)
        .into_iter()
        .map(|e| (e.key, e.label, entry_action(e.entry)))
        .collect()
}

/// What a row of the `≡` menu does here.
#[must_use]
pub const fn entry_action(entry: Entry) -> UiAction {
    match entry {
        Entry::Sheet(kind) => UiAction::Toggle(overlay_of(kind)),
        Entry::Help => UiAction::Help,
        Entry::Settings => UiAction::Settings,
        Entry::Quit => UiAction::Quit,
        Entry::Toss(slot) => UiAction::TossPotion(slot),
    }
}

/// The overlay a sheet is.
const fn overlay_of(kind: Kind) -> Overlay {
    match kind {
        Kind::Menu => Overlay::Menu,
        Kind::Deck => Overlay::Deck,
        Kind::Relics => Overlay::Relics,
        Kind::Info => Overlay::Info,
        Kind::Map => Overlay::Map,
        Kind::History => Overlay::History,
        Kind::Log => Overlay::Log,
        Kind::DrawPile => Overlay::DrawPile,
        Kind::DiscardPile => Overlay::DiscardPile,
        Kind::ExhaustPile => Overlay::ExhaustPile,
    }
}

/// The sheet an overlay is (`None` for the inspect overlay, which is the TUI's).
const fn kind_of(overlay: Overlay) -> Option<Kind> {
    Some(match overlay {
        Overlay::Menu => Kind::Menu,
        Overlay::Deck => Kind::Deck,
        Overlay::Relics => Kind::Relics,
        Overlay::Info => Kind::Info,
        Overlay::Map => Kind::Map,
        Overlay::History => Kind::History,
        Overlay::Log => Kind::Log,
        Overlay::DrawPile => Kind::DrawPile,
        Overlay::DiscardPile => Kind::DiscardPile,
        Overlay::ExhaustPile => Kind::ExhaustPile,
        Overlay::Inspect(_) => return None,
    })
}

/// What a button of the end screen does here.
#[must_use]
pub const fn end_action(action: EndAction) -> UiAction {
    match action {
        EndAction::Retry => UiAction::Retry,
        EndAction::NewSeed => UiAction::NewSeed,
        EndAction::Deck => UiAction::Toggle(Overlay::Deck),
        EndAction::Relics => UiAction::Toggle(Overlay::Relics),
        EndAction::Visited => UiAction::Toggle(Overlay::History),
        EndAction::Characters => UiAction::CharacterSelect,
        EndAction::Quit => UiAction::Quit,
    }
}

/// What the inspect overlay shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Inspect {
    /// A card of the fight: in the hand, or offered by a card choice.
    Card(CardId),
    Enemy(CreatureId),
    /// The `n`th (1-based) entry of the screen's menu.
    Item(usize),
    /// The potion in this belt slot.
    Potion(usize),
    /// The `i`th relic.
    Relic(usize),
}

/// The whole UI state.
#[derive(Clone, Debug)]
pub struct App {
    seed: String,
    ascension: u8,
    run: Run,
    ui: Ui,
    /// Digits typed so far for a multi-digit pick.
    number: String,
    error: Option<String>,
    /// An irreversible choice that was asked for and is waiting for its confirmation.
    confirm: Option<Confirm>,
    /// The rooms entered so far, oldest first.
    visits: Vec<Visit>,
    /// How many log lines the view is scrolled up from the newest.
    log_scroll: usize,
    /// The inspect overlay (the sheets are [`Ui`]'s).
    inspect: Option<Inspect>,
    overlay_scroll: usize,
    /// How many lines the out-of-combat screen is scrolled down.
    menu_scroll: usize,
    /// How many rows the map screen's grid is scrolled down from where it follows the player.
    map_scroll: isize,
    describer: Describer,
    /// Set when the player quits, or asks on the end screen (`c`) for the character select.
    leave: Option<Leave>,
    /// Every action applied, as its index in [`App::recordable_actions`] of the state it was
    /// applied to: with the seed, all a save needs to play the run back.
    actions: Vec<u32>,
    /// Whether the run moved on (or a new one started) since [`App::take_changed`] last asked.
    changed: bool,
    /// The first key of a two-key chord was pressed in an overlay: what the next number means.
    chord: Option<Chord>,
    /// The key being handled, so a confirmation knows which key confirms it.
    pressed: Option<KeyCode>,
    /// What the room just left did (`Rested: ♥ 30 → 56`, `Upgraded Strike → Strike+`), for
    /// the map; cleared when the next room is chosen.
    receipt: Vec<String>,
    /// What was bought in the shop being visited, for the receipt.
    bought: Vec<String>,
    /// The rewards taken on the rewards screen showing, in words.
    taken: Vec<String>,
    /// Where the keyboard focus was last put; `None` means the screen's default.
    focus: Option<Focus>,
    /// A press on a menu entry the shell carries out (the help, the settings).
    request: Option<UiAction>,
    /// The bot's advice and the number of actions the run had when it was asked.
    hint: Option<(usize, String)>,
}

pub use sts2_view::rooms::{PAGE, Paging};

impl App {
    /// A fresh run of `character` on `seed`.
    ///
    /// # Errors
    /// If the run cannot be created (an unsupported ascension, or a seed whose map the game
    /// cannot generate).
    pub fn new(character: Character, seed: &str, ascension: u8) -> Result<Self, RunError> {
        Self::with_text(character, seed, ascension, Rc::new(crate::text::discover()))
    }

    /// A fresh run that shows `text` (use [`Localization::empty`] for ids and names only).
    ///
    /// # Errors
    /// As [`App::new`].
    pub fn with_text(
        character: Character,
        seed: &str,
        ascension: u8,
        text: Rc<Localization>,
    ) -> Result<Self, RunError> {
        let run =
            Run::new(character, seed, ascension, UnlockState::all(), Act1::Random)?.with_events();
        Ok(Self::build(seed, ascension, run, text))
    }

    /// An app playing `run`, which was created from `seed` and `ascension`, showing ids and
    /// engine names only (no game text).
    #[must_use]
    pub fn with_run(seed: &str, ascension: u8, run: Run) -> Self {
        Self::build(seed, ascension, run, Rc::new(Localization::empty()))
    }

    /// [`App::with_run`] with game text.
    #[must_use]
    pub fn with_run_text(seed: &str, ascension: u8, run: Run, text: Rc<Localization>) -> Self {
        Self::build(seed, ascension, run, text)
    }

    fn build(seed: &str, ascension: u8, run: Run, text: Rc<Localization>) -> Self {
        let mut describer = Describer::new(text);
        describer.refresh(&run);
        let mut app = Self {
            seed: seed.to_owned(),
            ascension,
            run,
            ui: Ui::default(),
            number: String::new(),
            error: None,
            confirm: None,
            visits: Vec::new(),
            log_scroll: 0,
            inspect: None,
            overlay_scroll: 0,
            menu_scroll: 0,
            map_scroll: 0,
            describer,
            leave: None,
            actions: Vec::new(),
            changed: true,
            chord: None,
            pressed: None,
            receipt: Vec::new(),
            bought: Vec::new(),
            taken: Vec::new(),
            focus: None,
            request: None,
            hint: None,
        };
        app.record_visit();
        app
    }

    /// The run of `character` on `seed` at `ascension`, played on through `actions` (as
    /// [`App::actions`] lists them), showing `text`.
    ///
    /// # Errors
    /// If the run cannot start or an action is not legal where it is reached (the engine's
    /// actions changed since the list was made).
    pub fn replay(
        character: Character,
        seed: &str,
        ascension: u8,
        text: Rc<Localization>,
        actions: &[u32],
    ) -> Result<Self, ReplayError> {
        let mut app = Self::with_text(character, seed, ascension, text)
            .map_err(|e| ReplayError::Start(names::run_error(&e)))?;
        for (step, &index) in actions.iter().enumerate() {
            let action = usize::try_from(index)
                .ok()
                .and_then(|i| app.recordable_actions().into_iter().nth(i))
                .ok_or(ReplayError::Step(step))?;
            app.apply_one(action);
            if app.actions.len() != step + 1 {
                return Err(ReplayError::Step(step));
            }
        }
        app.error = None;
        Ok(app)
    }

    /// The actions applied so far, as [`App::replay`] takes them.
    #[must_use]
    pub fn actions(&self) -> &[u32] {
        &self.actions
    }

    /// Applies `action` as the key or click that makes it would (for a policy or a tool driving
    /// the app); one that cannot be taken now shows an error and changes nothing.
    pub fn apply(&mut self, action: RunAction) {
        if self.recordable_actions().contains(&action) {
            self.act(action);
        } else {
            self.error = Some("that cannot be done now".to_owned());
        }
    }

    /// What an action can be, in the order the action list numbers them: the legal actions,
    /// then the potion discards.
    #[must_use]
    pub fn recordable_actions(&self) -> Vec<RunAction> {
        let mut actions = self.run.legal_actions();
        actions.extend(self.run.discard_actions());
        actions
    }

    /// A digest of the game state (the run's, the fight's and what can be done next): two apps
    /// with the same digest are in the same place.
    #[must_use]
    pub fn digest(&self) -> u64 {
        use std::fmt::Write as _;
        let mut hash = Fnv(0xcbf2_9ce4_8422_2325);
        let _ = write!(
            hash,
            "{:?}{:?}{:?}",
            self.run.state(),
            self.run.combat().map(Combat::state),
            self.recordable_actions()
        );
        hash.0
    }

    /// Whether the run moved on or was replaced since the last call.
    pub const fn take_changed(&mut self) -> bool {
        std::mem::replace(&mut self.changed, false)
    }

    /// Shows `message` on the error line until the next action.
    pub fn notify(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Whether menu entries that throw something away ask first (on by default).
    pub const fn set_confirmations(&mut self, on: bool) {
        self.ui.set_confirmations(on);
    }

    /// The key bindings that apply now, the first match winning: what the input handler reads
    /// and the help shows.
    #[must_use]
    pub fn layers(&self) -> Vec<Layer> {
        if self.confirm.is_some() || self.ui.asking().is_some() {
            return vec![keys::CONFIRM];
        }
        let mut layers = Vec::new();
        match self.overlay() {
            Some(Overlay::Info | Overlay::Inspect(_)) => layers.push(keys::INSPECT),
            Some(Overlay::Menu) => layers.push(keys::MENU),
            Some(_) => {}
            None => {
                if self.open_reward().is_some() {
                    layers.push(keys::CARD_REWARD);
                }
            }
        }
        layers.push(keys::NAV);
        layers.push(keys::RUN);
        if self.overlay().is_some() {
            layers.push(keys::OVERLAY);
        } else if self.is_over() {
            layers.push(keys::END);
        } else {
            layers.push(keys::PICK);
        }
        layers
    }

    /// The rooms entered so far, oldest first.
    #[must_use]
    pub fn visits(&self) -> &[Visit] {
        &self.visits
    }

    /// Notes the room the run is in (once per floor) and keeps its HP and gold up to date.
    fn record_visit(&mut self) {
        let state = self.run.state();
        let (act, floor) = (state.act_index + 1, state.history.len());
        let what = match self.run.pending_decision() {
            RunDecision::Ancient(room) => Some(names::title_case(room.kind.id())),
            RunDecision::Combat(combat) => {
                let state = combat.state();
                let mut enemies: Vec<(String, usize)> = Vec::new();
                for &e in &state.enemies {
                    let name = names::creature(state, e);
                    match enemies.iter_mut().find(|(n, _)| *n == name) {
                        Some((_, count)) => *count += 1,
                        None => enemies.push((name, 1)),
                    }
                }
                let enemies: Vec<String> = enemies
                    .into_iter()
                    .map(|(n, count)| {
                        if count > 1 {
                            format!("{count}x {n}")
                        } else {
                            n
                        }
                    })
                    .collect();
                let kind = match self.run.state().room_type() {
                    Some(RoomType::Elite) => "Elite",
                    Some(RoomType::Boss) => "Boss",
                    _ => "Fight",
                };
                Some(format!("{kind}: {}", enemies.join(", ")))
            }
            RunDecision::Event(room) => Some(format!(
                "Event: {}",
                self.describer
                    .event_title(room.kind.id())
                    .unwrap_or_else(|| names::title_case(room.kind.id()))
            )),
            RunDecision::Shop(_) => Some("Shop".to_owned()),
            RunDecision::RestSite(_) => Some("Rest site".to_owned()),
            RunDecision::Treasure(_) => Some("Treasure".to_owned()),
            _ => None,
        };
        let hp = self.current_hp();
        let (max_hp, gold) = (state.max_hp, state.gold);
        let known = self
            .visits
            .last()
            .is_some_and(|v| v.act == act && v.floor == floor);
        if !known && let Some(what) = what {
            self.visits.push(Visit {
                act,
                floor,
                what,
                hp_in: hp,
                hp_out: hp,
                max_hp,
                gold_out: gold,
            });
        } else if let Some(last) = self.visits.last_mut() {
            last.hp_out = hp;
            last.max_hp = max_hp;
            last.gold_out = gold;
        }
    }

    #[must_use]
    pub fn seed(&self) -> &str {
        &self.seed
    }

    #[must_use]
    pub const fn run(&self) -> &Run {
        &self.run
    }

    /// The player's HP: a fight owns it while it runs.
    #[must_use]
    pub fn current_hp(&self) -> i32 {
        self.run.combat().map_or_else(
            || self.run.state().hp,
            |c| c.state().creature(c.state().player.creature).hp.max(0),
        )
    }

    /// The fields of the `i`th relic: the fight's while one runs (its counters move there).
    #[must_use]
    pub fn relic_data(&self, i: usize) -> RelicData {
        let relics = &self.run.state().relics;
        self.combat()
            .map(|c| &c.state().player.relics)
            .filter(|fight| fight.len() == relics.len())
            .map_or(relics[i].data, |fight| fight[i].data)
    }

    /// The fight in progress, if any.
    #[must_use]
    pub fn combat(&self) -> Option<&Combat> {
        self.run.combat()
    }

    /// The fight as the model's screen: what the fight is drawn from.
    #[must_use]
    pub fn fight_screen(&self) -> Option<sts2_view::model::Screen> {
        self.ui.screen(&self.run, &self.describer)
    }

    /// The bar above every screen: HP, gold, floor and the potion belt.
    #[must_use]
    pub fn top_bar(&self) -> sts2_view::model::TopBar {
        self.ui.top_bar(&self.run, &self.describer)
    }

    #[must_use]
    pub const fn input(&self) -> &Input {
        self.ui.mode()
    }

    /// Digits typed for a pick that is not confirmed yet.
    #[must_use]
    pub fn number(&self) -> &str {
        &self.number
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The question of the button pressed once, if one waits for its second press.
    #[must_use]
    pub fn confirm_prompt(&self) -> Option<String> {
        self.ui.asking().map_or_else(
            || self.confirm.as_ref().map(|c| format!("{}?", c.what)),
            |a| Some(format!("{}?", a.what)),
        )
    }

    /// The button pressed once, waiting: its number among the enabled entries and the label
    /// it shows in place of its own (`Leave 2 rewards behind? Enter again`, `Buy for 250?
    /// press 8`).
    #[must_use]
    pub fn confirm_row(&self) -> Option<(usize, String)> {
        let a = self.ui.asking()?;
        Some((a.entry, a.label()))
    }

    /// The question a potion's throw-away row in the `≡` menu asks in place once pressed.
    #[must_use]
    pub fn toss_row(&self, slot: usize) -> Option<String> {
        let c = self
            .confirm
            .as_ref()
            .filter(|c| c.action == RunAction::DiscardPotion(slot))?;
        let hint = match c.key {
            Some(KeyCode::Char(k)) if k != ' ' => format!("press {k}"),
            _ => "Enter again".to_owned(),
        };
        Some(format!("{}? {hint}", c.what))
    }

    /// The letters of the belt slots that hold a potion, as the potion row prints them, for
    /// the hint line: `z c`.
    #[must_use]
    pub fn potion_keys(&self) -> String {
        self.run
            .potions()
            .iter()
            .enumerate()
            .filter_map(|(slot, p)| p.map(|_| keys::potion_key(slot).to_string()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// What the room just left did, one line each (`Rested: ♥ 30 → 56`, `Upgraded Strike →
    /// Strike+`, `Took ◆ Vajra`, `Bought Inferno, Fire Potion`), until the next room is chosen.
    #[must_use]
    pub fn receipt(&self) -> Vec<String> {
        let mut lines = self.receipt.clone();
        if !self.bought.is_empty() {
            lines.push(format!("Bought {}", self.bought.join(", ")));
        }
        lines
    }

    #[must_use]
    pub fn log_scroll(&self) -> usize {
        if self.ui.sheet() == Some(Kind::Log) {
            self.ui.sheet_scroll()
        } else {
            self.log_scroll
        }
    }

    /// The enemy the last card or potion was aimed at in this fight.
    #[must_use]
    pub const fn last_target(&self) -> Option<CreatureId> {
        self.ui.last_target()
    }

    /// The rewards taken so far on the rewards screen showing, in words.
    #[must_use]
    pub fn taken(&self) -> &[String] {
        &self.taken
    }

    /// Whether the next number typed in the inspect overlay is an enemy's.
    #[must_use]
    pub fn inspecting_enemy(&self) -> bool {
        self.chord == Some(Chord::Enemy)
    }

    #[must_use]
    pub fn overlay(&self) -> Option<Overlay> {
        self.inspect
            .map(Overlay::Inspect)
            .or_else(|| self.ui.sheet().map(overlay_of))
    }

    /// The open sheet as a screen, for the drawer.
    #[must_use]
    pub fn sheet_screen(&self) -> Option<sts2_view::model::Screen> {
        self.ui.sheet_screen(&self.sheets_context())
    }

    fn sheets_context(&self) -> sheets::Context<'_> {
        sheets::Context {
            run: &self.run,
            describer: &self.describer,
            visits: &self.visits,
            tossing: self.confirm.as_ref().and_then(|c| match c.action {
                RunAction::DiscardPotion(slot) => self.toss_row(slot).map(|q| (slot, q)),
                _ => None,
            }),
        }
    }

    /// Which sheet is open.
    #[must_use]
    pub const fn sheet_kind(&self) -> Option<Kind> {
        self.ui.sheet()
    }

    /// How far the open sheet is scrolled: from its top, from its newest line for the log.
    #[must_use]
    pub const fn sheet_scroll(&self) -> usize {
        self.ui.sheet_scroll()
    }

    /// The `≡` menu's rows, each with what pressing it does.
    #[must_use]
    pub fn menu_rows(&self) -> Vec<(UiAction, sts2_view::model::Button)> {
        sheets::menu_rows(&self.sheets_context())
            .into_iter()
            .map(|r| (entry_action(r.entry), r.button))
            .collect()
    }

    const fn close_overlay(&mut self) {
        self.inspect = None;
        self.ui.close_sheet();
        self.overlay_scroll = 0;
    }

    #[must_use]
    pub const fn overlay_scroll(&self) -> usize {
        self.overlay_scroll
    }

    #[must_use]
    pub const fn menu_scroll(&self) -> usize {
        self.menu_scroll
    }

    /// Rows the map grid is scrolled from where it follows the player (negative: up).
    #[must_use]
    pub const fn map_scroll(&self) -> isize {
        self.map_scroll
    }

    /// Keeps the scroll positions within what the last frame could show, so scrolling back
    /// starts at once after scrolling past the end.
    pub fn clamp_scrolls(&mut self, hits: &Hits) {
        let clamp = |value: usize, pane| {
            hits.scroll_limit(pane).map_or(value, |(_, max)| {
                value.min(usize::try_from(max).unwrap_or(0))
            })
        };
        self.menu_scroll = clamp(self.menu_scroll, Pane::Menu);
        self.overlay_scroll = clamp(self.overlay_scroll, Pane::Overlay);
        if self.ui.sheet() == Some(Kind::Log) {
            let max = clamp(usize::MAX, Pane::Log);
            self.ui.clamp_sheet_scroll(max);
        } else if self.ui.sheet().is_some() {
            let max = clamp(usize::MAX, Pane::Overlay);
            self.ui.clamp_sheet_scroll(max);
        } else {
            self.log_scroll = clamp(self.log_scroll, Pane::Log);
        }
        if let Some((min, max)) = hits.scroll_limit(Pane::Map) {
            self.map_scroll = self.map_scroll.clamp(min, max.max(min));
        }
    }

    /// Fills in the game text for the screens.
    #[must_use]
    pub const fn describer(&self) -> &Describer {
        &self.describer
    }

    #[must_use]
    pub const fn should_quit(&self) -> bool {
        matches!(self.leave, Some(Leave::Quit))
    }

    /// Whether the player asked, on the end screen, for the character select.
    #[must_use]
    pub const fn wants_select(&self) -> bool {
        matches!(self.leave, Some(Leave::Select))
    }

    /// Whether the run has ended or stopped: the player died, the run is won, or a fight met
    /// content that is not ported.
    #[must_use]
    pub fn is_over(&self) -> bool {
        matches!(
            self.run.pending_decision(),
            RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
        )
    }

    /// The current screen's menu (empty in combat and on the end screens).
    #[must_use]
    pub fn menu(&self) -> Vec<MenuItem> {
        self.ui.room_items(&self.run, &self.describer)
    }

    /// What the incoming line showed when the last play of this turn began: for a card choice
    /// of the fight, which has no preview of its own.
    #[must_use]
    pub const fn turn_incoming(&self) -> Option<&IncomingPreview> {
        self.ui.turn_incoming()
    }

    /// The fight's keys for the help, as the groups that apply now: the group's title and its
    /// `(keys, what they do)` rows.
    #[must_use]
    pub fn fight_help(&self) -> Vec<(&'static str, &'static [(&'static str, &'static str)])> {
        let Some(combat) = self.combat() else {
            return Vec::new();
        };
        let turn = combat.status() == Status::PlayerTurn;
        view_ui::HELP
            .iter()
            .filter(|(title, _)| match *title {
                "Your turn" => turn,
                "Choosing a target" => matches!(self.input(), Input::Target(Pending::Card(_))),
                "Dragged card" => matches!(self.input(), Input::Armed(_)),
                "A potion waits" => matches!(self.input(), Input::Potion(_)),
                _ => true,
            })
            .copied()
            .collect()
    }

    /// The legal answers to the combat's pending card choice, in display order.
    #[must_use]
    pub fn decision_options(&self) -> Vec<Action> {
        view_ui::decision_options(&self.run)
    }

    /// The belt slots whose potion can be used now, with the actions that use them (one per
    /// target).
    #[must_use]
    pub fn potion_actions(&self) -> Vec<(usize, RunAction)> {
        view_ui::potion_actions(&self.run)
    }

    /// The shop section open as the shop's second level.
    #[must_use]
    pub fn open_section(&self) -> Option<menu::ShopSection> {
        self.ui.open_section(&self.run)
    }

    /// The card reward open as its own screen, while it is still there to answer.
    #[must_use]
    pub fn open_reward(&self) -> Option<usize> {
        self.ui.open_reward(&self.run)
    }

    /// The end of the run as the model's screen; `None` while the run goes on.
    #[must_use]
    pub fn end_screen(&self) -> Option<sts2_view::model::Screen> {
        sts2_view::rooms::end_screen(&self.run, &self.describer, &self.seed, &self.visits)
    }

    /// The room as the model's screen: what the room is drawn from; `None` for the screens
    /// still drawn from the menu's entries (the shop).
    #[must_use]
    pub fn room_screen(&self) -> Option<sts2_view::model::Screen> {
        self.ui
            .room_screen_with(&self.run, &self.describer, self.page(), &self.receipt())
    }

    /// The act map's grid, whatever room is on show (the enlarged map draws it).
    #[must_use]
    pub fn map_grid(&self) -> sts2_view::model::MapGrid {
        sts2_view::rooms::map_grid(&self.run, &self.menu())
    }

    /// How the screen's menu maps to the digit keys.
    #[must_use]
    pub fn paging(&self) -> Paging {
        Paging::of(&self.menu())
    }

    /// The page of the screen's list the focus is on (the page follows the focus; the main
    /// button counts as the last page).
    #[must_use]
    pub fn page(&self) -> usize {
        let paging = self.paging();
        match self.focused() {
            Some(Target::Button(UiAction::Pick(n))) => {
                paging.page_of(n).unwrap_or_else(|| paging.pages() - 1)
            }
            _ => 0,
        }
    }

    /// Moves the focus to the first row of the page `by` pages on (clamped), when the list
    /// is paged.
    fn turn_page(&mut self, by: isize) {
        let paging = self.paging();
        let pages = paging.pages();
        if pages <= 1 {
            return;
        }
        let page = (self.page().cast_signed() + by).clamp(0, (pages - 1).cast_signed());
        if let Some(first) = paging.first_of(page.cast_unsigned()) {
            self.focus_on(Target::Button(UiAction::Pick(first)));
        }
        self.menu_scroll = 0;
    }

    /// The entry a digit (`0` for ten) picks on the page the focus is on; past the list's end
    /// an index past the end, so the error names the digit.
    fn digit_entry(&self, digit: usize) -> usize {
        let paging = self.paging();
        let page = self.page();
        paging
            .pick(page, digit)
            .unwrap_or(page * PAGE + if digit == 0 { PAGE } else { digit })
    }

    /// Starts a new run, on the same seed or a fresh random one.
    pub fn restart(&mut self, same_seed: bool) {
        let seed = if same_seed {
            self.seed.clone()
        } else {
            random_seed()
        };
        let text = self.describer.localization();
        let character = self.run.state().character;
        let confirmations = self.ui.confirmations();
        match Self::with_text(character, &seed, self.ascension, text) {
            Ok(app) => {
                *self = app;
                self.ui.set_confirmations(confirmations);
            }
            Err(e) => self.error = Some(names::run_error(&e)),
        }
    }

    /// Handles one key press, by the binding [`App::layers`] gives it. The error line clears
    /// on any key, so no red text outlives the key that caused it.
    pub fn handle_key(&mut self, key: KeyCode) {
        self.error = None;
        // A button that asked takes the next key: its own again or Enter confirms, any other
        // puts it back.
        if self.ui.asking().is_some() {
            self.room(&view_key(key).map_or(ViewInput::Back, ViewInput::Key));
            return;
        }
        // The end screen's buttons print their keys.
        if self.is_over()
            && self.confirm.is_none()
            && self.overlay().is_none()
            && let Some(key) = view_key(key)
            && self.end(&ViewInput::Key(key))
        {
            return;
        }
        // The fight's keys are the fight's own (a selected potion takes every key); the rest
        // go by the bindings.
        let takes = self.confirm.is_none()
            && !self.is_over()
            && (self.overlay().is_none() || matches!(self.ui.mode(), Input::Potion(_)));
        if takes && self.tile_row_key(key) {
            return;
        }
        if takes
            && let Some(key) = view_key(key)
            && self.fight(&ViewInput::Key(key))
        {
            return;
        }
        let Some((_, binding)) = keys::lookup(&self.layers(), key, &[]) else {
            return;
        };
        self.pressed = Some(key);
        self.command(binding.command, key);
        self.pressed = None;
    }

    /// Runs `input` through the fight's state machine and carries out what it asks; `false`
    /// when the input is not the fight's.
    fn fight(&mut self, input: &ViewInput) -> bool {
        let effects = self.ui.input(&self.run, &self.describer, input);
        self.carry_out(effects)
    }

    /// Runs `input` through the room's state machine (the card reward on show, the button
    /// that asks); `false` when the input is not the room's.
    fn room(&mut self, input: &ViewInput) -> bool {
        let page = self.page();
        let effects = self.ui.room_input(&self.run, &self.describer, page, input);
        self.carry_out(effects)
    }

    /// Runs `input` through the end screen's buttons; `false` when it is none of them.
    fn end(&mut self, input: &ViewInput) -> bool {
        let effects = self.ui.end_input(input);
        self.carry_out(effects)
    }

    /// Does what `effects` ask; `false` when one passes the input on.
    fn carry_out(&mut self, effects: Vec<Effect>) -> bool {
        let mut handled = true;
        for effect in effects {
            match effect {
                Effect::Apply(action) => self.act(action),
                Effect::Error(message) => self.error = Some(message),
                Effect::End(action) => self.perform(end_action(action)),
                Effect::Menu(entry) => self.perform(entry_action(entry)),
                Effect::Pass => handled = false,
            }
        }
        handled
    }

    /// A click or key action that is the fight's, as the model's input.
    fn fight_action(&mut self, action: UiAction) {
        let hand = |app: &Self, card| {
            app.combat()
                .and_then(|c| c.state().player.hand.iter().position(|&h| h == card))
                .map(view_ui::ids::row)
        };
        let input = match action {
            UiAction::EndTurn => Some(ViewInput::Tap(view_ui::ids::END_TURN)),
            UiAction::Pick(n) if n > 0 => Some(ViewInput::Tap(view_ui::ids::row(n - 1))),
            UiAction::Card(card) => hand(self, card).map(ViewInput::Tap),
            UiAction::Arm(card) => hand(self, card).map(ViewInput::Arm),
            UiAction::Target(creature) => Some(ViewInput::Tap(view_ui::ids::creature(creature))),
            UiAction::PlayArmed => Some(ViewInput::Drop),
            UiAction::UsePotion(slot) => Some(ViewInput::Tap(view_ui::ids::potion(slot))),
            _ => None,
        };
        if let Some(input) = input {
            self.fight(&input);
        }
    }

    /// Carries out what a key's binding says.
    fn command(&mut self, command: Command, key: KeyCode) {
        match command {
            Command::Ui(action) => self.perform(action),
            // The pressed button's own key again confirms it; any other key puts it back.
            Command::Confirm => {
                let same = self.confirm.as_ref().is_some_and(|c| c.key == Some(key));
                self.perform(if same { UiAction::Yes } else { UiAction::No });
            }
            Command::Digit => {
                if let KeyCode::Char(c) = key {
                    self.number.push(c);
                    self.try_pick();
                }
            }
            Command::Focus(by) => self.focus_move(by),
            Command::Activate => self.activate(),
            // Backspace erases a digit.
            Command::Erase => {
                self.number.pop();
            }
            Command::Inspect => self.inspect_key(key),
            Command::InspectEnemy => {
                self.number.clear();
                self.chord = Some(Chord::Enemy);
            }
            Command::SkipCard => self.skip_card(),
            Command::Unknown => {
                self.error = Some("no such key · ? lists the keys".to_owned());
            }
            _ => {}
        }
    }

    /// Carries out `action`, whether a key or a click asked for it. Anything that is not legal
    /// now shows an error or does nothing; the engine is never asked for an illegal action.
    pub fn perform(&mut self, action: UiAction) {
        if self.ui.asking().is_some() {
            // The same button again (or Yes) takes it; anything else puts it back.
            self.room(&match action {
                UiAction::Yes => ViewInput::Primary,
                UiAction::Pick(n) => ViewInput::Tap(ButtonId(u32::try_from(n).unwrap_or(0))),
                _ => ViewInput::Back,
            });
            return;
        }
        if let Some(pending) = self.confirm.take() {
            // The same button again (or Yes) takes it; anything else puts it back.
            if action == UiAction::Yes || action == pending.again {
                // A potion thrown away from the `≡` menu closes the menu on it.
                if matches!(pending.action, RunAction::DiscardPotion(_)) {
                    self.close_overlay();
                }
                self.act(pending.action);
            }
            return;
        }
        // A selected potion: the same potion again (or Yes) uses it, `t` throws it away,
        // another potion's row selects that one; anything else puts it back.
        if let Input::Potion(slot) = *self.ui.mode() {
            let input = match action {
                UiAction::Yes => Some(ViewInput::Primary),
                UiAction::UsePotion(s) => Some(ViewInput::Tap(view_ui::ids::potion(s))),
                UiAction::TossPotion(s) if s == slot => Some(ViewInput::Key(ViewKey::Char('t'))),
                UiAction::Scroll(_) | UiAction::ScrollMap(_) => None,
                _ => Some(ViewInput::Back),
            };
            if let Some(input) = input {
                self.fight(&input);
                return;
            }
        }
        match action {
            // The run is saved after every action: quitting loses nothing.
            UiAction::Quit => self.leave = Some(Leave::Quit),
            // The bot runs on the shell's thread: the shell takes the request.
            UiAction::Hint => self.request = Some(UiAction::Hint),
            UiAction::Scroll(by) if by < 0 => self.scroll_up(by.unsigned_abs()),
            UiAction::Scroll(by) => self.scroll_down(by.unsigned_abs()),
            UiAction::ScrollMap(by) => self.map_scroll = self.map_scroll.saturating_add(by),
            UiAction::Toggle(overlay) => self.toggle(overlay),
            UiAction::Back if self.overlay().is_some() => {
                self.sheet_input(&ViewInput::Back);
                self.close_overlay();
                self.chord = None;
            }
            // A potion's throw-away row of the `≡` menu: it asks in place first.
            UiAction::TossPotion(slot) if self.overlay() == Some(Overlay::Menu) => {
                self.ask_toss(slot);
            }
            // A reachable node clicked on the enlarged map: travel there.
            UiAction::Travel(coord) if self.overlay() == Some(Overlay::Map) => {
                if self
                    .run
                    .legal_actions()
                    .contains(&RunAction::ChooseNode(coord))
                {
                    self.close_overlay();
                    self.travel(coord);
                }
            }
            UiAction::Back => self.back(),
            _ if self.overlay().is_some() => {}
            UiAction::CharacterSelect if self.is_over() => self.leave = Some(Leave::Select),
            UiAction::Retry if self.is_over() => self.restart(true),
            UiAction::NewSeed if self.is_over() => self.restart(false),
            _ if self.is_over() => {}
            UiAction::Pick(n) if n > 0 && self.combat().is_none() => {
                self.cancel();
                self.room(&ViewInput::Tap(ButtonId(u32::try_from(n).unwrap_or(0))));
            }
            UiAction::EndTurn
            | UiAction::Pick(_)
            | UiAction::Card(_)
            | UiAction::Arm(_)
            | UiAction::Target(_)
            | UiAction::PlayArmed
            | UiAction::UsePotion(_) => self.fight_action(action),
            UiAction::Travel(coord) => self.travel(coord),
            _ => {}
        }
    }

    /// The list the keyboard focus moves in now, if the screen has choices.
    fn screen(&self) -> Option<Screen> {
        if self.confirm.is_some() {
            return None;
        }
        match self.overlay() {
            Some(Overlay::Menu) => return Some(Screen::OverlayMenu),
            Some(_) => return None,
            None => {}
        }
        if self.is_over() {
            return Some(Screen::End);
        }
        // The fight keeps its own focus ([`Ui`]).
        if self.combat().is_some() {
            return None;
        }
        let kind = match self.run.pending_decision() {
            RunDecision::Ancient(_) => 1,
            RunDecision::Event(_) => 2,
            RunDecision::Map(_) => 3,
            RunDecision::Rewards(_) if self.open_reward().is_some() => 10,
            RunDecision::Rewards(_) => 4,
            RunDecision::CardSelect(_) => 5,
            RunDecision::Bundles(_) => 6,
            RunDecision::RestSite(_) => 7,
            RunDecision::Treasure(_) => 8,
            RunDecision::Shop(_) => 9,
            _ => 0,
        };
        Some(Screen::Menu(self.run.state().history.len(), kind))
    }

    /// Whether the fight owns the keyboard focus: a fight on show with no list over it.
    fn fight_focus(&self) -> bool {
        self.combat().is_some() && self.confirm.is_none() && self.overlay().is_none()
    }

    /// What the keyboard focus can rest on, top to bottom: the potions, the hand and End Turn
    /// in a fight; the targets while a card waits for one; the choices of a room; the buttons
    /// of a menu. Enter presses the focused one.
    #[must_use]
    pub fn focusables(&self) -> Vec<Target> {
        if self.fight_focus() {
            return self
                .ui
                .items(&self.run)
                .into_iter()
                .map(item_target)
                .collect();
        }
        let Some(screen) = self.screen() else {
            return Vec::new();
        };
        let menu = || {
            let n = self.menu().iter().filter(|i| i.enabled).count();
            (1..=n).map(|n| Target::Button(UiAction::Pick(n)))
        };
        match screen {
            Screen::OverlayMenu => self
                .menu_rows()
                .into_iter()
                .filter(|(_, row)| row.state == ButtonState::Enabled)
                .map(|(action, _)| Target::Button(action))
                .collect(),
            Screen::End => sts2_view::rooms::END_BUTTONS
                .iter()
                .map(|&(_, _, action)| Target::Button(end_action(action)))
                .collect(),
            Screen::Menu(..) => menu().collect(),
        }
    }

    /// What the keyboard focus rests on: where it was put, if that is still there; its place
    /// in the list if what was there went (the card played); else the screen's default: the
    /// first card that can be played, the last target aimed at, the first choice.
    #[must_use]
    pub fn focused(&self) -> Option<Target> {
        if self.fight_focus() {
            return self.ui.focused(&self.run).map(item_target);
        }
        let items = self.focusables();
        if items.is_empty() {
            return None;
        }
        let screen = self.screen()?;
        if let Some(focus) = self.focus.filter(|f| f.screen == screen) {
            if items.contains(&focus.target) {
                return Some(focus.target);
            }
            return items.get(focus.pos.min(items.len() - 1)).copied();
        }
        items.first().copied()
    }

    /// Moves the focus `by` places (wrapping round); with nothing to focus, scrolls the open
    /// list instead.
    fn focus_move(&mut self, by: isize) {
        let items = self.focusables();
        let Some(screen) = self.screen().filter(|_| !items.is_empty()) else {
            if self.overlay().is_some() {
                self.perform(UiAction::Scroll(by));
            }
            return;
        };
        let at = self
            .focused()
            .and_then(|f| items.iter().position(|t| *t == f))
            .unwrap_or(0);
        let n = items.len().cast_signed();
        let pos = (at.cast_signed() + by).rem_euclid(n).cast_unsigned();
        self.focus = Some(Focus {
            screen,
            target: items[pos],
            pos,
        });
        self.menu_scroll = 0;
        self.error = None;
    }

    /// `↑`/`↓` on a hand drawn as card tiles: the card above or below on the screen (`↓` from
    /// the bottom row goes on past the hand, `↑` from the top row back before it), as that many
    /// steps of the fight's focus. `false` when the key is not one of these.
    fn tile_row_key(&mut self, key: KeyCode) -> bool {
        let columns = crate::ui::combat::tiles::columns();
        let down = match key {
            KeyCode::Down => true,
            KeyCode::Up => false,
            _ => return false,
        };
        let Some(Target::Card(card)) = self.focused().filter(|_| columns > 1) else {
            return false;
        };
        let Some(hand) = self.combat().map(|c| &c.state().player.hand) else {
            return false;
        };
        let Some(at) = hand.iter().position(|&h| h == card) else {
            return false;
        };
        let (steps, step) = if down {
            // The card below, else past the last card.
            let below = (at + columns).min(hand.len());
            (below - at, ViewKey::Right)
        } else if at >= columns {
            (columns, ViewKey::Left)
        } else {
            (at + 1, ViewKey::Left)
        };
        for _ in 0..steps {
            self.fight(&ViewInput::Key(step));
        }
        true
    }

    /// Puts the focus on `target` (a click landed on it), if it can be focused now.
    pub fn focus_on(&mut self, target: Target) {
        if self.fight_focus() {
            if let Some(item) = target_item(target) {
                self.ui.focus_on(&self.run, item);
            }
            return;
        }
        let items = self.focusables();
        if let (Some(screen), Some(pos)) = (self.screen(), items.iter().position(|t| *t == target))
        {
            self.focus = Some(Focus {
                screen,
                target,
                pos,
            });
        }
    }

    /// Enter: picks the number typed, else presses what the focus rests on.
    fn activate(&mut self) {
        if !self.number.is_empty() {
            self.try_pick_now();
            return;
        }
        let Some(target) = self.focused() else {
            return;
        };
        match target {
            Target::Button(action @ (UiAction::Help | UiAction::Settings)) => {
                self.request = Some(action);
            }
            Target::Button(action) => self.perform(action),
            _ => {}
        }
    }

    /// Shows `text` as the bot's advice for the position the run is in now; it goes with the
    /// next action.
    pub fn set_hint(&mut self, text: String) {
        self.hint = Some((self.actions.len(), text));
    }

    /// The bot's advice for the position the run is in, if it was asked for.
    #[must_use]
    pub fn hint(&self) -> Option<&str> {
        self.hint
            .as_ref()
            .filter(|(at, _)| *at == self.actions.len())
            .map(|(_, text)| text.as_str())
    }

    /// A press on a menu entry for the shell to carry out (the help, the settings), once.
    pub const fn take_request(&mut self) -> Option<UiAction> {
        self.request.take()
    }

    /// Scrolls the open list back toward its start by `by` lines (the log: toward older lines).
    fn scroll_up(&mut self, by: usize) {
        if self.ui.sheet().is_some() {
            self.ui.scroll_sheet(-by.cast_signed());
        } else if self.overlay().is_some() {
            self.overlay_scroll = self.overlay_scroll.saturating_sub(by);
        } else if self.combat().is_some() {
            self.log_scroll = self.log_scroll.saturating_add(by);
        } else if self.paging().paged() {
            self.turn_page(-1);
        } else {
            self.menu_scroll = self.menu_scroll.saturating_sub(by);
        }
    }

    fn scroll_down(&mut self, by: usize) {
        if self.ui.sheet().is_some() {
            self.ui.scroll_sheet(by.cast_signed());
        } else if self.overlay().is_some() {
            self.overlay_scroll = self.overlay_scroll.saturating_add(by);
        } else if self.combat().is_some() {
            self.log_scroll = self.log_scroll.saturating_sub(by);
        } else if self.paging().paged() {
            self.turn_page(1);
        } else {
            self.menu_scroll = self.menu_scroll.saturating_add(by);
        }
    }

    /// Opens an overlay or closes it when it is the open one. The sheets are [`Ui`]'s; the
    /// fight's are refused outside a fight.
    fn toggle(&mut self, overlay: Overlay) {
        self.chord = None;
        self.number.clear();
        let Some(kind) = kind_of(overlay) else {
            return;
        };
        self.inspect = None;
        self.overlay_scroll = 0;
        if kind.fight_only() {
            self.log_scroll = 0;
        }
        let effects = self.ui.toggle_sheet(&self.run, kind);
        self.carry_out(effects);
    }

    /// Hands `input` to the open sheet and carries out what it asks.
    fn sheet_input(&mut self, input: &ViewInput) {
        let effects = self.ui.sheet_input(&self.run, &self.describer, input);
        self.carry_out(effects);
    }

    /// A digit (or Enter) in the inspect overlay: inspects that card, enemy or menu entry once
    /// the number cannot grow into another.
    fn inspect_key(&mut self, key: KeyCode) {
        let count = self.inspect_count();
        match key {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.number.push(c);
                match self.number.parse::<usize>() {
                    Ok(n) if n == 0 || n.saturating_mul(10) > count => self.inspect_now(),
                    Ok(_) => {}
                    Err(_) => self.number.clear(),
                }
            }
            _ => self.inspect_now(),
        }
    }

    /// How many things the inspect overlay's numbers pick from.
    fn inspect_count(&self) -> usize {
        match self.combat() {
            Some(c) if self.inspecting_enemy() => c.state().enemies.len(),
            Some(c) if c.status() == Status::Decision => self.decision_options().len(),
            Some(c) => c.state().player.hand.len(),
            None => self.menu().iter().filter(|i| i.enabled).count(),
        }
    }

    /// Inspects what the typed number names.
    fn inspect_now(&mut self) {
        let Ok(n) = std::mem::take(&mut self.number).parse::<usize>() else {
            return;
        };
        let index = if n == 0 { 10 } else { n };
        let enemy = self.chord.take() == Some(Chord::Enemy);
        let what = match self.combat() {
            Some(c) if enemy => c.state().enemies.get(index - 1).map(|&e| Inspect::Enemy(e)),
            Some(c) if c.status() == Status::Decision => {
                match self.decision_options().get(index - 1) {
                    Some(Action::Resolve(sts2_core::combat::Decision::Pick(card))) => {
                        Some(Inspect::Card(*card))
                    }
                    _ => None,
                }
            }
            Some(c) => c
                .state()
                .player
                .hand
                .get(index - 1)
                .map(|&card| Inspect::Card(card)),
            None => {
                let index = self.digit_entry(n);
                (index <= self.inspect_count()).then_some(Inspect::Item(index))
            }
        };
        match what {
            Some(what) => {
                self.ui.close_sheet();
                self.inspect = Some(what);
                self.overlay_scroll = 0;
                self.error = None;
            }
            None => self.error = Some(format!("nothing to inspect at {index}")),
        }
    }

    /// Opens the detail sheet of `what` (a right click on its row).
    pub fn open_inspect(&mut self, what: Inspect) {
        self.ui.close_sheet();
        self.inspect = Some(what);
        self.overlay_scroll = 0;
        self.error = None;
    }

    /// The card reward's Skip row (`s`, printed on it).
    fn skip_card(&mut self) {
        self.cancel();
        self.room(&ViewInput::Key(ViewKey::Char('s')));
    }

    fn cancel(&mut self) {
        self.ui.cancel();
        self.number.clear();
    }

    /// Esc: puts a half-chosen card or potion back; else closes the card reward's screen,
    /// cancels a card selection the engine lets the player cancel, or leaves the shop.
    fn back(&mut self) {
        if !self.number.is_empty() {
            self.cancel();
            return;
        }
        if self.fight(&ViewInput::Back) {
            return;
        }
        self.room(&ViewInput::Back);
    }

    /// A throw-away row of the `≡` menu pressed: it asks in place, the same press again (or
    /// Enter) throws the potion away.
    fn ask_toss(&mut self, slot: usize) {
        let action = RunAction::DiscardPotion(slot);
        if !self.run.discard_actions().contains(&action) {
            self.error = Some(format!("no potion in slot {}", slot + 1));
            return;
        }
        let name = self
            .run
            .potions()
            .get(slot)
            .copied()
            .flatten()
            .map_or_else(String::new, names::potion);
        self.confirm = Some(Confirm {
            action,
            what: format!("Throw away {name}"),
            entry: 0,
            again: UiAction::TossPotion(slot),
            key: self.pressed,
        });
    }

    /// Resolves the typed digit at once: `1`-`9`, `0` the tenth; a longer list is paged, the
    /// digit picking on the page the focus is on.
    fn try_pick(&mut self) {
        if self.number.parse::<usize>().is_ok() {
            self.try_pick_now();
        } else {
            self.cancel();
        }
    }

    /// Resolves the typed number as it stands.
    fn try_pick_now(&mut self) {
        let Ok(n) = self.number.parse::<usize>() else {
            return;
        };
        self.number.clear();
        // Keys are 1-based; a lone `0` addresses the tenth item.
        let index = if n == 0 { 10 } else { n };
        // In the `≡` menu a digit presses a potion's throw-away row.
        if self.overlay() == Some(Overlay::Menu) {
            self.ask_toss(index - 1);
        } else {
            let digit = u8::try_from(n).unwrap_or(u8::MAX);
            self.room(&ViewInput::Key(ViewKey::Digit(digit)));
        }
    }

    fn travel(&mut self, coord: MapCoord) {
        let action = RunAction::ChooseNode(coord);
        if self.run.legal_actions().contains(&action) {
            self.act(action);
        } else {
            self.error = Some("that room cannot be reached from here".to_owned());
        }
    }

    /// What `action` would do to the deck, the belt or the HP, for the room's receipt on the
    /// map (the names come from the action's card or relic; the HP from before and after).
    fn receipt_of(&self, action: &RunAction) -> Option<Receipt> {
        let state = self.run.state();
        match (action, self.run.pending_decision()) {
            (RunAction::ChooseNode(_), _) => Some(Receipt::NewRoom),
            (RunAction::RestOption(i), RunDecision::RestSite(options)) => options
                .get(*i)
                .filter(|o| o.kind == sts2_core::run::RestOptionKind::Heal)
                .map(|_| Receipt::Rested),
            (RunAction::SelectCard(n), RunDecision::CardSelect(select)) => {
                let card = *select.cards(&state.deck).get(*n)?;
                match select.purpose {
                    SelectPurpose::Smith | SelectPurpose::Upgrade => {
                        let mut up = card;
                        up.upgrade();
                        Some(Receipt::Line(format!(
                            "Upgraded {} → {}",
                            names::run_card(card),
                            names::run_card(up)
                        )))
                    }
                    SelectPurpose::Remove { .. }
                    | SelectPurpose::ShopRemoval { .. }
                    | SelectPurpose::Cook => {
                        Some(Receipt::Line(format!("Removed {}", names::run_card(card))))
                    }
                    _ => None,
                }
            }
            (RunAction::TakeRelic, RunDecision::Treasure(Some(id))) => {
                Some(Receipt::Line(format!("Took ◆ {}", names::relic(id))))
            }
            (RunAction::BuyCard(i), RunDecision::Shop(shop)) => {
                Some(Receipt::Bought(names::run_card(shop.cards.get(*i)?.card?)))
            }
            (RunAction::BuyRelic(i), RunDecision::Shop(shop)) => {
                Some(Receipt::Bought(names::relic(shop.relics.get(*i)?.id?)))
            }
            (RunAction::BuyPotion(i), RunDecision::Shop(shop)) => {
                Some(Receipt::Bought(names::potion(shop.potions.get(*i)?.id?)))
            }
            _ => None,
        }
    }

    /// Notes what the action just applied did, for the room's receipt on the map.
    fn note_receipt(&mut self, receipt: Option<Receipt>, hp_before: i32) {
        match receipt {
            Some(Receipt::NewRoom) => {
                self.receipt.clear();
                self.bought.clear();
            }
            Some(Receipt::Rested) => self
                .receipt
                .push(format!("Rested: ♥ {hp_before} → {}", self.current_hp())),
            Some(Receipt::Line(line)) => self.receipt.push(line),
            Some(Receipt::Bought(name)) => self.bought.push(name),
            None => {}
        }
    }

    /// Applies `action`, then what follows on its own (a reward screen left empty leaves); each
    /// is recorded, so a replay applies the list as it is ([`Self::apply_one`]).
    fn act(&mut self, action: RunAction) {
        self.apply_one(action);
        while let Some(next) = sts2_view::ui::follow_up(&self.run) {
            self.apply_one(next);
        }
    }

    /// Applies `action` alone, which must come from `legal_actions`.
    fn apply_one(&mut self, action: RunAction) {
        self.cancel();
        let taking = match (&action, self.run.pending_decision()) {
            (RunAction::ClaimReward(i), RunDecision::Rewards(rewards)) => {
                rewards.get(*i).map(menu::reward_name)
            }
            (RunAction::PickCard { reward, card }, RunDecision::Rewards(rewards)) => {
                match rewards.get(*reward) {
                    Some(sts2_core::run::Reward::Card(cards)) => {
                        cards.cards.get(*card).map(|c| names::run_card(*c))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let receipt = self.receipt_of(&action);
        let hp_before = self.current_hp();
        self.ui.before_apply(&self.run, &action);
        let index = self.recordable_actions().iter().position(|a| *a == action);
        let screen = self.screen();
        let result = self.run.apply(action);
        match result {
            Ok(()) => {
                self.note_receipt(receipt, hp_before);
                // Every action comes from `recordable_actions`; one that did not would make
                // the save fail to load at this step (`u32::MAX`) rather than play on wrong.
                self.actions.push(
                    index
                        .and_then(|i| u32::try_from(i).ok())
                        .unwrap_or(u32::MAX),
                );
                self.changed = true;
                self.error = None;
                self.log_scroll = 0;
                self.menu_scroll = 0;
                self.map_scroll = 0;
                self.describer.refresh(&self.run);
                self.record_visit();
                if matches!(self.run.pending_decision(), RunDecision::Rewards(_)) {
                    self.taken.extend(taking);
                } else {
                    self.taken.clear();
                }
                self.ui.after_apply(&self.run);
                // Another screen starts with its own default focus.
                if self.screen() != screen {
                    self.focus = None;
                }
            }
            Err(e) => self.error = Some(names::run_error(&e)),
        }
    }
}

/// The fight's focus item as the hit layer's target.
const fn item_target(item: Item) -> Target {
    match item {
        Item::Potion(slot) => Target::Potion(slot),
        Item::Card(card) => Target::Card(card),
        Item::Choice(n, card) => Target::Choice(n, card),
        Item::Creature(id) => Target::Creature(id),
        Item::EndTurn => Target::Button(UiAction::EndTurn),
        Item::Done(n) => Target::Button(UiAction::Pick(n)),
    }
}

/// [`item_target`]'s inverse, for the targets the fight can focus.
const fn target_item(target: Target) -> Option<Item> {
    match target {
        Target::Potion(slot) => Some(Item::Potion(slot)),
        Target::Card(card) => Some(Item::Card(card)),
        Target::Choice(n, card) => Some(Item::Choice(n, card)),
        Target::Creature(id) => Some(Item::Creature(id)),
        Target::Button(UiAction::EndTurn) => Some(Item::EndTurn),
        Target::Button(UiAction::Pick(n)) => Some(Item::Done(n)),
        _ => None,
    }
}

/// A key press as the model's key, when the model has one.
const fn view_key(key: KeyCode) -> Option<ViewKey> {
    Some(match key {
        KeyCode::Char(' ') => ViewKey::Space,
        KeyCode::Char(c @ '0'..='9') => ViewKey::Digit(c as u8 - b'0'),
        KeyCode::Char(c) => ViewKey::Char(c),
        KeyCode::Enter => ViewKey::Enter,
        KeyCode::Esc => ViewKey::Esc,
        KeyCode::Backspace => ViewKey::Backspace,
        KeyCode::Up => ViewKey::Up,
        KeyCode::Down => ViewKey::Down,
        KeyCode::Left => ViewKey::Left,
        KeyCode::Right => ViewKey::Right,
        KeyCode::PageUp => ViewKey::PageUp,
        KeyCode::PageDown => ViewKey::PageDown,
        KeyCode::Tab => ViewKey::Tab,
        _ => return None,
    })
}

/// The first key of a two-key chord in an overlay: what the next number means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chord {
    /// `e` in the inspect overlay: an enemy's number.
    Enemy,
}

/// What an action does to the run, for the room's receipt ([`App::receipt`]).
enum Receipt {
    /// The next room was chosen: the receipt starts afresh.
    NewRoom,
    /// Rested: the HP before and after make the line.
    Rested,
    Line(String),
    /// Something bought: the shop's purchases make one line.
    Bought(String),
}

/// FNV-1a, fed by `write!`: a digest that is the same on every platform and build.
struct Fnv(u64);

impl std::fmt::Write for Fnv {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        for b in s.bytes() {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        Ok(())
    }
}

/// A fresh seed from the clock; only the TUI entry points call this, never the engine.
#[must_use]
pub fn random_seed() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ0123456789";
    let mut n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    (0..10)
        .map(|_| {
            let idx = usize::try_from(n % ALPHABET.len() as u128).unwrap_or(0);
            n /= ALPHABET.len() as u128;
            char::from(ALPHABET[idx])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "RL2S2";

    fn app() -> App {
        App::new(Character::Necrobinder, SEED, 0).expect("run")
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.handle_key(KeyCode::Char(c));
        }
    }

    /// Takes the first choice that works on each screen until a fight starts.
    fn to_fight(app: &mut App) {
        for _ in 0..30 {
            if app.combat().is_some() {
                return;
            }
            let options = app.menu().iter().filter(|i| i.enabled).count();
            let mut next = None;
            for n in 1..=options {
                let mut copy = app.clone();
                press(&mut copy, &n.to_string());
                if copy.error().is_none() {
                    next = Some(copy);
                    break;
                }
            }
            *app = next.expect("a choice that works");
        }
        panic!("no fight within 30 choices");
    }

    #[test]
    fn a_run_starts_on_the_neow_event() {
        let app = app();
        assert!(matches!(
            app.run().pending_decision(),
            RunDecision::Ancient(_)
        ));
        assert_eq!(app.menu().len(), 3, "Neow offers three options");
        assert!(!app.is_over());
    }

    #[test]
    fn scripted_fight_reaches_an_end() {
        let mut app = app();
        to_fight(&mut app);
        // Play the first card three times (extra presses just show an error), then end the turn.
        for _ in 0..200 {
            if app.combat().is_none() {
                break;
            }
            press(&mut app, "111e");
        }
        assert!(app.combat().is_none(), "fight should end within 200 rounds");
    }

    #[test]
    fn illegal_input_shows_an_error_and_changes_nothing() {
        let mut app = app();
        let before = app.run().clone();
        press(&mut app, "9");
        assert_eq!(app.error(), Some("no option 9"));
        assert_eq!(app.run(), &before);
        app.handle_key(KeyCode::Char('y'));
        assert_eq!(app.error(), Some("no such key · ? lists the keys"));
        app.handle_key(KeyCode::Char('z'));
        assert_eq!(app.error(), Some("no potion in slot 1"));
    }

    #[test]
    fn playing_without_energy_is_rejected() {
        let mut app = app();
        to_fight(&mut app);
        press(&mut app, "111");
        let after_three = app.run().clone();
        press(&mut app, "1");
        // The engine's reason, in the game's words when its text is installed.
        let error = app.error().expect("rejected");
        assert!(
            error.starts_with("cannot play: ") && error.contains("Energy"),
            "{error}"
        );
        assert_eq!(app.run(), &after_three);
    }

    #[test]
    fn leaving_rewards_behind_asks_first() {
        let mut app = app();
        to_fight(&mut app);
        for _ in 0..200 {
            if app.combat().is_none() {
                break;
            }
            press(&mut app, "111e");
        }
        assert!(
            matches!(app.run().pending_decision(), RunDecision::Rewards(_)),
            "the fight is won"
        );
        let items = app.menu();
        let leave = items.last().expect("a Leave entry");
        assert_eq!(leave.action, RunAction::Proceed);
        assert!(leave.warning.is_some(), "gold is still unclaimed");
        let before = app.run().clone();
        let n = items.iter().filter(|i| i.enabled).count();
        press(&mut app, &n.to_string());
        assert_eq!(app.run(), &before, "nothing happens yet");
        assert!(
            app.confirm_prompt()
                .is_some_and(|p| p.starts_with("Leave without taking"))
        );
        assert_eq!(
            app.confirm_row().map(|(entry, _)| entry),
            Some(n),
            "the pressed row asks in place"
        );
        press(&mut app, "z");
        assert!(app.confirm_prompt().is_none());
        assert_eq!(app.run(), &before, "any other key puts it back");
        press(&mut app, &n.to_string());
        press(&mut app, &n.to_string());
        assert_ne!(app.run(), &before, "the same key again leaves");
    }

    #[test]
    fn the_history_lists_each_room_once() {
        let mut app = app();
        to_fight(&mut app);
        press(&mut app, "111e");
        let visits = app.visits();
        assert!(visits[0].what.starts_with("Neow"), "{visits:?}");
        let fights: Vec<_> = visits
            .iter()
            .filter(|v| v.what.starts_with("Fight: "))
            .collect();
        assert_eq!(fights.len(), 1, "{visits:?}");
        assert!(fights[0].hp_out <= fights[0].hp_in);
        press(&mut app, "h");
        assert_eq!(app.overlay(), Some(Overlay::History));
    }

    #[test]
    fn restart_keeps_or_changes_the_seed() {
        let mut app = app();
        press(&mut app, "1");
        app.restart(true);
        assert_eq!(app.seed(), SEED);
        assert!(matches!(
            app.run().pending_decision(),
            RunDecision::Ancient(_)
        ));
        app.restart(false);
        assert_ne!(app.seed(), SEED);
    }

    #[test]
    fn overlays_toggle_and_swallow_other_keys() {
        let mut app = app();
        press(&mut app, "d");
        assert_eq!(app.overlay(), Some(Overlay::Deck));
        press(&mut app, "1");
        assert_eq!(app.error(), None);
        press(&mut app, "v");
        assert_eq!(app.overlay(), Some(Overlay::Relics));
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.overlay(), None);
    }

    #[test]
    fn throwing_away_needs_a_potion() {
        let mut app = app();
        press(&mut app, "t");
        assert_eq!(app.error(), Some("no such key · ? lists the keys"));
        app.handle_key(KeyCode::Tab);
        press(&mut app, "1");
        assert_eq!(app.error(), Some("no potion in slot 1"));
        assert_eq!(app.input(), &Input::Idle);
        assert_eq!(app.overlay(), Some(Overlay::Menu));
    }

    #[test]
    fn quit_key_quits_at_once() {
        let mut app = app();
        press(&mut app, "q");
        assert!(app.should_quit(), "the run is saved, so nothing is asked");
    }

    #[test]
    fn replaying_the_actions_reaches_the_same_state() {
        let mut app = app();
        to_fight(&mut app);
        press(&mut app, "111e1");
        assert_ne!(app.actions(), &[] as &[u32]);
        let copy = App::replay(
            Character::Necrobinder,
            SEED,
            0,
            Rc::new(Localization::empty()),
            app.actions(),
        )
        .expect("replays");
        assert_eq!(copy.run(), app.run());
        assert_eq!(copy.digest(), app.digest());
        assert_eq!(copy.visits(), app.visits());
        assert_eq!(
            App::replay(
                Character::Necrobinder,
                SEED,
                0,
                Rc::new(Localization::empty()),
                &[999]
            )
            .err(),
            Some(ReplayError::Step(0))
        );
    }
}
