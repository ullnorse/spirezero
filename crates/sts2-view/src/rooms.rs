//! The rooms that are a list of choices, as [`Screen`]s: Neow and the ancients, events, rewards,
//! card selections, bundles, rest sites and treasure rooms.
//!
//! The choices are [`menu::items`](crate::menu::items) (the labels over the engine's legal
//! actions); this module turns them into rows. A row's [`ButtonId`] is its number among the
//! enabled entries (the 1-based `n` that picking `n` means), a disabled row's is far above, so a
//! tap names a row and never a position. Which rows a page shows and which key picks each is
//! [`Paging`]. A button that cannot be undone shows its question in place ([`Asking`]).
//!
//! Nothing here applies an action: [`Ui::room_input`](crate::ui::Ui::room_input) does the
//! asking and the picking, and the run applies what it returns.

use std::fmt::Write as _;

use sts2_core::combat::{CombatState, Event, PromptText};
use sts2_core::content::relics::RelicKind;
use sts2_core::run::{
    CardSelect, Reward, Room, RoomType, Run, RunAction, RunCard, RunDecision, SelectPrompt,
    SelectPurpose, shop,
};

use crate::describe::{Describer, EventText};
use crate::menu::{self, MenuItem, ShopSection};
use crate::model::{Block, Button, ButtonId, ButtonState, CardFace, InfoRef, Key, Page, Screen};
use crate::names;
use crate::text::{self, Rich, RichSpan, Role};

mod map;

pub use map::grid as map_grid;

/// The rows a page of a long list holds: the keys `1`-`9` and `0`.
pub const PAGE: usize = 10;

/// The id of a row that cannot be picked (and is none of the numbered ones).
const DISABLED: u32 = 10_000;
/// The id of the Back button on the shop's second level.
pub const BACK: ButtonId = ButtonId(9_000);

/// How a screen's numbered choices map to the digit keys.
///
/// Up to ten entries one digit picks at once (`0` the tenth). A longer list is paged:
/// PgUp/PgDn turn the pages, a digit picks on the page the focus is on, and the screen's main
/// button (Leave, Done) is `0` on every page, the pages then holding nine rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paging {
    /// The enabled entries (1-based, as a row's id counts them) in page order.
    rows: Vec<usize>,
    /// The main button's entry, while the list is paged (it is on no page).
    main: Option<usize>,
    size: usize,
}

impl Paging {
    /// The paging of `items`.
    #[must_use]
    pub fn of(items: &[MenuItem]) -> Self {
        let enabled: Vec<&MenuItem> = items.iter().filter(|i| i.enabled).collect();
        let n = enabled.len();
        let main = enabled
            .iter()
            .position(|i| matches!(i.action, RunAction::Proceed | RunAction::ConfirmSelect))
            .map(|p| p + 1)
            .filter(|_| n > PAGE);
        let size = if main.is_some() { PAGE - 1 } else { PAGE };
        Self {
            rows: (1..=n).filter(|g| Some(*g) != main).collect(),
            main,
            size,
        }
    }

    /// Whether the list is paged.
    #[must_use]
    pub const fn paged(&self) -> bool {
        self.rows.len() > self.size
    }

    #[must_use]
    pub fn pages(&self) -> usize {
        self.rows.len().div_ceil(self.size).max(1)
    }

    /// The page entry `n` is on; `None` for the main button.
    #[must_use]
    pub fn page_of(&self, n: usize) -> Option<usize> {
        self.rows
            .iter()
            .position(|&r| r == n)
            .map(|p| p / self.size)
    }

    /// The key that picks entry `n`: a digit, `0` the tenth of its page or the main button.
    #[must_use]
    pub fn key(&self, n: usize) -> String {
        if Some(n) == self.main {
            return "0".to_owned();
        }
        match self
            .rows
            .iter()
            .position(|&r| r == n)
            .map_or(0, |p| p % self.size + 1)
        {
            PAGE => "0".to_owned(),
            k => k.to_string(),
        }
    }

    /// The entry `digit` picks on `page`, if there is one.
    #[must_use]
    pub fn pick(&self, page: usize, digit: usize) -> Option<usize> {
        match (digit, self.main) {
            (0, Some(main)) => Some(main),
            (0, None) => self.rows.get(page * self.size + PAGE - 1).copied(),
            (d, _) => self.rows.get(page * self.size + d - 1).copied(),
        }
    }

    /// The first entry of `page`.
    #[must_use]
    pub fn first_of(&self, page: usize) -> Option<usize> {
        self.rows.get(page * self.size).copied()
    }
}

/// A button pressed once that cannot be undone, waiting for its second press.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
    pub action: RunAction,
    /// The question, without its `?`.
    pub what: String,
    /// The entry's number among the enabled entries.
    pub entry: usize,
    /// The key that pressed it, if a key did.
    pub key: Option<Key>,
}

impl Asking {
    /// The row's label while it asks: `Leave 2 rewards behind? Enter again`, `Buy for 250?
    /// press 8`.
    #[must_use]
    pub fn label(&self) -> String {
        let hint = match self.key {
            Some(Key::Digit(d)) => format!("press {d}"),
            Some(Key::Char(c)) if c != ' ' => format!("press {c}"),
            _ => "Enter again".to_owned(),
        };
        format!("{}? {hint}", self.what)
    }
}

/// What the client holds about the room's screen.
#[derive(Clone, Copy, Debug, Default)]
pub struct View<'a> {
    /// The card reward (its index among the rewards) open as its own screen.
    pub open: Option<usize>,
    /// The shop section open as the shop's second level.
    pub section: Option<ShopSection>,
    /// The whole shop on one level ([`menu::shop_flat`]) instead of its two.
    pub flat_shop: bool,
    /// The button pressed once, waiting.
    pub asking: Option<&'a Asking>,
    /// The page of a long list the focus is on.
    pub page: usize,
    /// What the room just left did, one line each, shown on the map until the next room.
    pub receipt: &'a [String],
}

/// Whether [`screen`] builds `decision`.
#[must_use]
pub const fn routed(decision: &RunDecision<'_>) -> bool {
    matches!(
        decision,
        RunDecision::Map(_)
            | RunDecision::Ancient(_)
            | RunDecision::Event(_)
            | RunDecision::Rewards(_)
            | RunDecision::CardSelect(_)
            | RunDecision::Bundles(_)
            | RunDecision::RestSite(_)
            | RunDecision::Treasure(_)
            | RunDecision::Shop(_)
            | RunDecision::Unsupported(_)
    )
}

/// The room as a screen; `None` for a decision that is not a list of choices this module
/// builds (a fight, the map, the shop).
#[must_use]
pub fn screen(run: &Run, describer: &Describer, view: &View<'_>) -> Option<Screen> {
    let decision = run.pending_decision();
    if !routed(&decision) {
        return None;
    }
    let items = view_items(run, describer, view);
    let paging = Paging::of(&items);
    if matches!(decision, RunDecision::Map(_)) {
        return Some(map::screen(run, describer, view, &items, &paging));
    }
    let (heading, mut lines) = describe(run, describer, view.open);
    let (heading, aside) = shop_title(run, view, heading);
    // A finished event's prompt (its rewards, a selection) shows the event's last page above.
    if !matches!(decision, RunDecision::Event(_)) {
        let mut finished = finished_event(run, describer);
        // The event's title is the screen's already.
        if finished.first().is_some_and(|l| l.text() == heading) {
            finished.remove(0);
        }
        if !finished.is_empty() {
            finished.push(Rich::default());
            lines.splice(0..0, finished);
        }
    }
    let upgrade = upgrade_select(&decision);
    let mut rows = Vec::new();
    let mut primary = back_button(&decision, view);
    let mut number = 0;
    // The page each list row is on: a row with no key goes with the keyed row before it.
    let mut pages = Vec::new();
    let mut last_page = 0;
    for (index, item) in items.into_iter().enumerate() {
        let n = item.enabled.then(|| {
            number += 1;
            number
        });
        let id = n.map_or_else(
            || ButtonId(DISABLED + u32::try_from(index).unwrap_or(0)),
            |n| ButtonId(u32::try_from(n).unwrap_or(u32::MAX)),
        );
        let mut key = n.map(|n| paging.key(n));
        let asking = view.asking.filter(|a| Some(a.entry) == n);
        if let Some(n) = n
            && primary.is_none()
            && matches!(item.action, RunAction::Proceed | RunAction::ConfirmSelect)
        {
            let label = asking.map_or(item.label, Asking::label);
            primary = Some(Button {
                key: key.as_deref().and_then(key_of),
                state: if asking.is_some() {
                    ButtonState::Picked
                } else {
                    ButtonState::Enabled
                },
                confirm: asking.map(|a| plain(a.label())),
                ..Button::row(u32::try_from(n).unwrap_or(0), plain(label))
            });
            continue;
        }
        if let Some(asking) = asking {
            pages.push(n.and_then(|n| paging.page_of(n)).unwrap_or(last_page));
            rows.push(Button {
                key: key.as_deref().and_then(key_of),
                state: ButtonState::Picked,
                confirm: Some(plain(asking.label())),
                ..Button::row(id.0, plain(asking.label()))
            });
            continue;
        }
        // The card reward's Skip has the letter the help names.
        if matches!(item.action, RunAction::SkipReward(_)) && item.open.is_none() && n.is_some() {
            key = Some("s".to_owned());
        }
        last_page = n.and_then(|n| paging.page_of(n)).unwrap_or(last_page);
        pages.push(last_page);
        rows.push(row(run, describer, &item, id, key.as_deref(), upgrade));
    }
    let page = paging.paged().then(|| {
        let first = pages.iter().position(|&p| p == view.page).unwrap_or(0);
        Page {
            index: view.page,
            count: paging.pages(),
            first,
            len: pages.iter().filter(|&&p| p == view.page).count(),
        }
    });
    lines.extend(nothing_to_buy(run, view, &rows).map(|why| role(Role::Muted, why)));
    let mut blocks = Vec::new();
    if !lines.is_empty() {
        blocks.push(Block::Text(lines));
    }
    blocks.push(Block::List {
        frame: true,
        rows,
        page,
    });
    Some(Screen {
        top: Some(crate::topbar::build(run, describer, None)),
        title: Some(plain(heading)),
        aside,
        blocks,
        primary,
        sheet: None,
        status: None,
    })
}

/// The room's choices as `view` lays them out (a card reward open, the shop's level).
fn view_items(run: &Run, describer: &Describer, view: &View<'_>) -> Vec<MenuItem> {
    let items = menu::items(run, describer, view.open);
    menu::shop_items_for(run, items, view.section, view.flat_shop)
}

/// Why a shop section's rows are all dim, or the dim rows are a dead end.
fn nothing_to_buy(run: &Run, view: &View<'_>, rows: &[Button]) -> Option<&'static str> {
    let section = view.section?;
    if !matches!(run.pending_decision(), RunDecision::Shop(_))
        || rows.is_empty()
        || !rows.iter().all(|r| matches!(r.state, ButtonState::Dim(_)))
    {
        return None;
    }
    let belt_full = run.potions().iter().all(Option::is_some);
    Some(if section == ShopSection::Potions && belt_full {
        "Your potion belt is full: throw one away (Tab menu) to make room."
    } else {
        "You cannot afford any of these."
    })
}

/// The shop's second level has no Leave; its way on is Back (Esc).
fn back_button(decision: &RunDecision<'_>, view: &View<'_>) -> Option<Button> {
    (view.section.is_some() && matches!(decision, RunDecision::Shop(_))).then(|| Button {
        key: Some(Key::Esc),
        ..Button::row(BACK.0, plain("Back"))
    })
}

/// The shop's title (the open section's name on its second level) and its gold at the right;
/// any other room's heading as it is.
fn shop_title(run: &Run, view: &View<'_>, heading: String) -> (String, Option<Rich>) {
    if !matches!(run.pending_decision(), RunDecision::Shop(_)) {
        return (heading, None);
    }
    let title = view.section.map_or(heading, |s| s.label().to_owned());
    (title, Some(price(run.state().gold, false)))
}

/// The end of the run as a screen: DEFEAT, VICTORY, or STOPPED.
///
/// STOPPED is where the engine has not ported what came next. The screen tells the story of
/// the run in a few lines (what killed you, the rooms, the fights, the deck), and the ways on
/// are buttons ([`END_BUTTONS`], numbered from 1).
#[must_use]
pub fn end_screen(
    run: &Run,
    describer: &Describer,
    seed: &str,
    visits: &[Visit],
) -> Option<Screen> {
    let s = run.state();
    let (word, word_role, mut notes) = match run.pending_decision() {
        RunDecision::Victory => (
            "VICTORY",
            Role::Heal,
            vec!["The Spire is conquered.".to_owned()],
        ),
        RunDecision::Stopped(unsupported) => (
            "STOPPED",
            Role::Buff,
            vec![format!("This content is not ported yet: {unsupported}")],
        ),
        RunDecision::GameOver => ("DEFEAT", Role::Attack, Vec::new()),
        _ => return None,
    };
    if word == "DEFEAT"
        && let Some(cause) = cause(run, visits)
    {
        notes.push(cause);
    }
    notes.push(String::new());
    notes.push(format!(
        "{} · Ascension {} · seed {seed}",
        s.character.name(),
        s.ascension,
    ));
    notes.push(counts(visits));
    notes.push(format!(
        "◉ {} · {} cards · {} relics",
        s.gold,
        s.deck.len(),
        s.relics.len(),
    ));
    let count = |label: &str| match label {
        "Deck" => Some(s.deck.len()),
        "Relics" => Some(s.relics.len()),
        "Rooms visited" => Some(visits.len()),
        _ => None,
    };
    let rows = END_BUTTONS
        .iter()
        .enumerate()
        .map(|(i, &(key, label, _))| Button {
            key: Some(Key::Char(key)),
            right: count(label).map(|n| role(Role::Body, &n.to_string())),
            ..Button::row(u32::try_from(i + 1).unwrap_or(0), plain(label))
        })
        .collect();
    Some(Screen {
        top: Some(crate::topbar::build(run, describer, None)),
        title: Some(Rich(vec![RichSpan {
            fx: None,
            role: word_role,
            bold: true,
            text: word.to_owned(),
        }])),
        aside: Some(role(
            Role::Body,
            &format!("Act {} · Floor {}", s.act_index + 1, s.total_floor()),
        )),
        blocks: vec![
            Block::Text(notes.iter().map(|n| role(Role::Body, n)).collect()),
            Block::List {
                frame: true,
                rows,
                page: None,
            },
        ],
        ..Screen::default()
    })
}

/// `Bygone Effigy: Tackle 4×3`: who dealt the last damage that cost the player HP in
/// `events`, and the move it came with (its hits on the player). `None` when the last HP was
/// lost to no creature (a card, a relic, a curse).
fn killer(state: &CombatState, events: &[Event]) -> Option<String> {
    let me = state.player.creature;
    let (at, dealer) = events.iter().enumerate().rev().find_map(|(i, e)| match e {
        Event::Damage {
            dealer: Some(d),
            receiver,
            hp_lost,
            ..
        } if *receiver == me && *hp_lost > 0 => Some((i, *d)),
        _ => None,
    })?;
    let name = names::creature(state, dealer);
    let Some(start) = events[..=at]
        .iter()
        .rposition(|e| matches!(e, Event::MonsterMove { monster, .. } if *monster == dealer))
    else {
        return Some(name);
    };
    let Event::MonsterMove { name: id, .. } = &events[start] else {
        return Some(name);
    };
    // The move's hits on the player, as the log counts them: `4×3`, `2+5`.
    let hits: Vec<i32> = events[start..]
        .iter()
        .filter_map(|e| match e {
            Event::Damage {
                dealer: Some(d),
                receiver,
                blocked,
                hp_lost,
            } if *d == dealer && *receiver == me => Some(blocked + hp_lost),
            _ => None,
        })
        .collect();
    let mut what = names::monster_move(id);
    if let Some(&first) = hits.first() {
        if hits.len() > 1 && hits.iter().all(|&h| h == first) {
            write!(what, " {first}×{}", hits.len()).expect("a String takes any write");
        } else {
            let each: Vec<String> = hits.iter().map(ToString::to_string).collect();
            write!(what, " {}", each.join("+")).expect("a String takes any write");
        }
    }
    Some(format!("{name}: {what}"))
}

/// What ended the run, from the fight's events while the engine kept them
/// ([`Run::ended_combat`]), else from the room the player died in.
#[must_use]
pub fn cause(run: &Run, visits: &[Visit]) -> Option<String> {
    if let Some(combat) = run.ended_combat() {
        return Some(killer(combat.state(), combat.events()).map_or_else(
            || "Slain by no enemy: the last HP went to a card, a relic or a curse.".to_owned(),
            |who| format!("Slain by {who}."),
        ));
    }
    let last = visits.last()?;
    let fight = ["Fight: ", "Elite: ", "Boss: "]
        .iter()
        .find_map(|p| last.what.strip_prefix(p));
    Some(fight.map_or_else(
        || {
            format!(
                "Died in {}.",
                last.what.strip_prefix("Event: ").unwrap_or(&last.what)
            )
        },
        |enemies| format!("Slain by {enemies}."),
    ))
}

/// `7 rooms · 4 fights · 1 elite · 2 events`, from the rooms visited.
fn counts(visits: &[Visit]) -> String {
    let count = |prefixes: &[&str]| {
        visits
            .iter()
            .filter(|v| prefixes.iter().any(|p| v.what.starts_with(p)))
            .count()
    };
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let parts = [
        plural(visits.len(), "room", "rooms"),
        plural(count(&["Fight: ", "Elite: ", "Boss: "]), "fight", "fights"),
        plural(count(&["Elite: "]), "elite", "elites"),
        plural(count(&["Event: "]), "event", "events"),
    ];
    parts.join(" · ")
}

/// A key as a row prints it: a digit, or a letter.
fn key_of(text: &str) -> Option<Key> {
    let mut chars = text.chars();
    let c = chars.next().filter(|_| chars.next().is_none())?;
    Some(
        c.to_digit(10)
            .map_or(Key::Char(c), |d| Key::Digit(u8::try_from(d).unwrap_or(0))),
    )
}

/// One room of the run so far, for the history and the end-of-run summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visit {
    /// The act (1-based) and the floor in it.
    pub act: usize,
    pub floor: usize,
    /// What the room was: `Fight: Jaw Worm` (`Elite: `, `Boss: ` for those), `Event: ...`,
    /// `Shop`.
    pub what: String,
    /// HP when the room was entered, and when the run last looked (HP leaving it).
    pub hp_in: i32,
    pub hp_out: i32,
    pub max_hp: i32,
    pub gold_out: i32,
}

/// What a button of the run's end screen asks the client to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EndAction {
    /// A new run on the same seed, or on a fresh one.
    Retry,
    NewSeed,
    /// The lists.
    Deck,
    Relics,
    Visited,
    /// Back to the character select.
    Characters,
    Quit,
}

/// The end screen's buttons: the key, the label and what the button does.
pub const END_BUTTONS: [(char, &str, EndAction); 7] = [
    ('r', "Retry this seed", EndAction::Retry),
    ('n', "New seed", EndAction::NewSeed),
    ('d', "Deck", EndAction::Deck),
    ('v', "Relics", EndAction::Relics),
    ('h', "Rooms visited", EndAction::Visited),
    ('c', "Character select", EndAction::Characters),
    ('q', "Quit", EndAction::Quit),
];

/// The most rows of a choice's text under its name.
pub const DETAIL_ROWS: usize = 2;

/// An entry as a row: a card as a card row, a reward by its glyph and name, anything else by its
/// label with its text under it.
fn row(
    run: &Run,
    describer: &Describer,
    item: &MenuItem,
    id: ButtonId,
    key: Option<&str>,
    upgrade: bool,
) -> Button {
    let key = key.and_then(key_of);
    let state = if item.enabled {
        ButtonState::Enabled
    } else {
        ButtonState::Dim(String::new())
    };
    if let Some((section, n)) = item.section {
        return Button {
            key,
            state,
            right: Some(role(Role::Body, &section.count_label(n))),
            label: Rich(vec![RichSpan {
                fx: None,
                role: Role::Plain,
                bold: true,
                text: item.label.clone(),
            }]),
            ..Button::row(id.0, Rich::default())
        };
    }
    if let Some(card) = menu::item_card(run, &item.action) {
        // A selection's picked card: lit, ticked, not to be picked again.
        let picked = matches!(item.action, RunAction::DeselectCard(_));
        let face = if upgrade {
            upgraded_face(describer, card)
        } else {
            card_face(describer, card)
        };
        let right = match (&item.action, run.pending_decision()) {
            (RunAction::BuyCard(i), RunDecision::Shop(shop)) => shop
                .cards
                .get(*i)
                .map(|c| price(shop::price(run.state(), c.base_cost), c.on_sale)),
            _ => None,
        };
        return Button {
            key,
            state: if picked { ButtonState::Picked } else { state },
            right: picked.then(|| role(Role::Key, "✓")).or(right).or_else(|| {
                (item.copies > 1).then(|| role(Role::Body, &format!("×{}", item.copies)))
            }),
            card: Some(face),
            ..Button::row(id.0, plain(names::run_card(card)))
        };
    }
    let line = glyph_line(run, item);
    let right = shop_price(run, &item.action);
    if let Some(label) = line {
        return Button {
            key,
            state,
            info: item_info(run, &item.action),
            // Only on the reward's own row: the Skip row inside its screen has just seen them.
            detail: reward_cards(run, &item.action)
                .filter(|_| item.open.is_some())
                .into_iter()
                .collect(),
            ..Button {
                right,
                ..Button::row(id.0, label)
            }
        };
    }
    let label = match &item.action {
        // A rest site's Leave is there once you have rested (the phone hides it until then).
        RunAction::Proceed
            if !item.enabled
                && matches!(run.pending_decision(), RunDecision::RestSite(_))
                && !describer.game_wording() =>
        {
            format!("{} (once you have rested)", item.label)
        }
        // The price is at the right: the label without it.
        RunAction::RemoveCard if right.is_some() => "Remove a card".to_owned(),
        _ => item.label.clone(),
    };
    let detail = item
        .detail
        .as_deref()
        .map(|d| vec![plain(text::plain(d).replace('\n', " "))])
        .unwrap_or_default();
    Button {
        key,
        state,
        detail,
        right,
        label: Rich(vec![RichSpan {
            fx: None,
            role: Role::Plain,
            bold: true,
            text: label,
        }]),
        ..Button::row(id.0, Rich::default())
    }
}

/// A reward or shop item by its glyph and name; `None` for an entry shown by its label.
fn glyph_line(run: &Run, item: &MenuItem) -> Option<Rich> {
    let named = |glyph: &str, glyph_role: Role, name: String| {
        Rich(vec![
            RichSpan {
                fx: None,
                role: glyph_role,
                bold: false,
                text: format!("{glyph} "),
            },
            RichSpan {
                fx: None,
                role: Role::Plain,
                bold: true,
                text: name,
            },
        ])
    };
    match (&item.action, run.pending_decision()) {
        (RunAction::SkipReward(i), RunDecision::Rewards(rewards)) if item.open.is_some() => {
            match rewards.get(*i) {
                Some(Reward::Card(_)) => Some(Rich(vec![RichSpan {
                    fx: None,
                    role: Role::Plain,
                    bold: true,
                    text: card_reward_label(run, rewards, *i),
                }])),
                _ => None,
            }
        }
        (RunAction::ClaimReward(i), RunDecision::Rewards(rewards)) => match rewards.get(*i) {
            Some(Reward::Gold(n)) => Some(named("◉", Role::Energy, format!("{n} gold"))),
            Some(Reward::Potion(id)) => Some(named("◈", Role::Buff, names::potion(id))),
            Some(Reward::Relic(id)) => Some(named("◆", Role::Title, names::relic(id))),
            Some(Reward::WaxRelic(id)) => Some(named(
                "◆",
                Role::Title,
                format!("{} (wax)", names::relic(id)),
            )),
            _ => None,
        },
        (RunAction::SkipReward(_), _) => Some(role(Role::Body, &item.label)),
        (action, RunDecision::Shop(shop)) => shop_line(shop, action, &named),
        _ => None,
    }
}

/// `Card reward`; a second one in a fight names the relic that added it (`PrayerWheel` in a
/// normal fight, `WhiteStar` in an elite), as the extra row is otherwise a puzzling repeat.
fn card_reward_label(run: &Run, rewards: &[Reward], index: usize) -> String {
    let extra = rewards[..index]
        .iter()
        .any(|r| matches!(r, Reward::Card(_)));
    let source = match run.state().room_type() {
        Some(RoomType::Monster) => RelicKind::PrayerWheel,
        Some(RoomType::Elite) => RelicKind::WhiteStar,
        _ => return "Card reward".to_owned(),
    };
    if extra && run.state().relics.iter().any(|r| r.kind == source) {
        format!("Card reward ({})", source.name())
    } else {
        "Card reward".to_owned()
    }
}

/// What a card reward offers, by name, for the line under its row: opening it costs nothing,
/// so the names save the trip when the answer is plainly "skip".
fn reward_cards(run: &Run, action: &RunAction) -> Option<Rich> {
    let (RunAction::SkipReward(i), RunDecision::Rewards(rewards)) =
        (action, run.pending_decision())
    else {
        return None;
    };
    let Some(Reward::Card(cards)) = rewards.get(*i) else {
        return None;
    };
    let names: Vec<String> = cards.cards.iter().map(|&c| names::run_card(c)).collect();
    Some(role(Role::Body, &names.join(" · ")))
}

/// The relic or potion a reward or shop row offers, for a client's detail sheet.
fn item_info(run: &Run, action: &RunAction) -> Option<InfoRef> {
    match (action, run.pending_decision()) {
        (RunAction::ClaimReward(i), RunDecision::Rewards(rewards)) => match rewards.get(*i)? {
            Reward::Potion(id) => Some(InfoRef::Potion((*id).to_owned())),
            Reward::Relic(id) | Reward::WaxRelic(id) => Some(InfoRef::Relic((*id).to_owned())),
            _ => None,
        },
        (RunAction::BuyRelic(i), RunDecision::Shop(shop)) => {
            Some(InfoRef::Relic(shop.relics.get(*i)?.id?.to_owned()))
        }
        (RunAction::BuyPotion(i), RunDecision::Shop(shop)) => {
            Some(InfoRef::Potion(shop.potions.get(*i)?.id?.to_owned()))
        }
        _ => None,
    }
}

/// A shop relic or potion by its glyph and name.
fn shop_line(
    shop: &sts2_core::run::Shop,
    action: &RunAction,
    named: &dyn Fn(&str, Role, String) -> Rich,
) -> Option<Rich> {
    match action {
        RunAction::BuyRelic(i) => shop.relics.get(*i).map(|r| {
            named(
                "◆",
                Role::Title,
                r.id.map_or_else(|| "sold".to_owned(), names::relic),
            )
        }),
        RunAction::BuyPotion(i) => shop.potions.get(*i).map(|p| {
            named(
                "◈",
                Role::Buff,
                p.id.map_or_else(|| "sold".to_owned(), names::potion),
            )
        }),
        _ => None,
    }
}

/// The price at the right of a shop relic, potion or removal row.
fn shop_price(run: &Run, action: &RunAction) -> Option<Rich> {
    let RunDecision::Shop(shop) = run.pending_decision() else {
        return None;
    };
    let state = run.state();
    match action {
        RunAction::BuyRelic(i) => shop
            .relics
            .get(*i)
            .filter(|r| r.id.is_some())
            .map(|r| price(shop::price(state, r.base_cost), false)),
        RunAction::BuyPotion(i) => shop
            .potions
            .get(*i)
            .filter(|p| p.id.is_some())
            .map(|p| price(shop::price(state, p.base_cost), false)),
        RunAction::RemoveCard if !shop.removal_used => {
            Some(price(shop::price(state, shop.removal_base_cost), false))
        }
        _ => None,
    }
}

/// `price` as the shop shows it, at the right of its row: `75 ◉`, `SALE 37 ◉`.
fn price(price: i32, sale: bool) -> Rich {
    let mut spans = Vec::new();
    if sale {
        spans.push(span(Role::Buff, "SALE ".to_owned()));
    }
    spans.push(span(Role::Energy, format!("{price} ◉")));
    Rich(spans)
}

/// A deck, reward or shop card in parts: its effect from the game's text, else its type.
#[must_use]
pub fn card_face(describer: &Describer, card: RunCard) -> CardFace {
    let combat = card.to_combat();
    let kind = combat
        .map_or("", |c| crate::combat::type_word(c.kind.card_type()))
        .to_owned();
    let rarity = combat
        .map_or("", |c| crate::combat::rarity_word(c.kind.rarity()))
        .to_owned();
    let frame = combat
        .map_or("colorless", |c| {
            crate::combat::frame_color(card.id, c.kind.card_type())
        })
        .to_owned();
    let (effect, full) = describer.run_card(card).as_deref().map_or_else(
        || (role(Role::Body, &kind), role(Role::Body, &kind)),
        |markup| {
            (
                text::inline(&crate::combat::joined(markup), Role::Body),
                text::inline(markup, Role::Body),
            )
        },
    );
    CardFace {
        card: 0,
        art: card.id.to_owned(),
        cost: names::run_card_cost(card).unwrap_or_default(),
        name: names::run_card(card),
        kind,
        rarity,
        frame,
        effect,
        text: full,
        glow: false,
        aims: false,
        before: None,
        upgrade: None,
    }
}

/// A deck card as the deck sheet shows it: its face with the card upgraded beside it, when it
/// can be.
#[must_use]
pub fn deck_face(describer: &Describer, card: RunCard) -> CardFace {
    let mut face = card_face(describer, card);
    if card.is_upgradable() {
        let mut up = upgraded_face(describer, card);
        up.before = None;
        face.upgrade = Some(Box::new(up));
    }
    face
}

/// A smith's row: the whole card upgraded, the words it changes marked, with the card as it is
/// now beside it; the card as it is when it cannot be upgraded.
pub(crate) fn upgraded_face(describer: &Describer, card: RunCard) -> CardFace {
    let now = card_face(describer, card);
    if !card.is_upgradable() {
        return now;
    }
    let mut up = card;
    up.upgrade();
    let mut face = card_face(describer, up);
    face.effect = text::mark_changes(face.effect, &now.effect.text());
    face.text = text::mark_changes(face.text, &now.text.text());
    face.before = Some(Box::new(now));
    face
}

/// Only the words `after` changes in `before` (`8 → 10 · 2 → 3`), the new ones lit; the whole of
/// `after` when the texts differ in shape. A compact line for an upgrade selection's row.
#[must_use]
pub fn changes_text(before: &str, after: &str) -> Rich {
    let old: Vec<&str> = before.split_whitespace().collect();
    let new: Vec<&str> = after.split_whitespace().collect();
    if old.len() != new.len() {
        return role(Role::Body, &after.replace('\n', " "));
    }
    let mut spans = Vec::new();
    for (a, b) in old.iter().zip(&new).filter(|(a, b)| a != b) {
        if !spans.is_empty() {
            spans.push(span(Role::Body, " · ".to_owned()));
        }
        spans.push(span(Role::Body, format!("{a} ")));
        spans.push(span(Role::Key, format!("→ {b}")));
    }
    if spans.is_empty() {
        return role(Role::Body, after);
    }
    Rich(spans)
}

/// What `after` changes in `before`, word by word (`Deal 6 → 9 damage.`), grey with the new
/// words lit; `after` alone when the texts differ in shape.
#[must_use]
pub fn diff_text(before: &str, after: &str) -> Rich {
    let old: Vec<&str> = before.split_whitespace().collect();
    let new: Vec<&str> = after.split_whitespace().collect();
    if old.len() != new.len() {
        return role(Role::Body, &after.replace('\n', " "));
    }
    let mut spans = Vec::new();
    for (i, (a, b)) in old.iter().zip(&new).enumerate() {
        let gap = if i == 0 { "" } else { " " };
        if a == b {
            spans.push(span(Role::Body, format!("{gap}{b}")));
        } else {
            spans.push(span(Role::Body, format!("{gap}{a} ")));
            spans.push(span(Role::Key, format!("→ {b}")));
        }
    }
    Rich(spans)
}

/// Whether the selection upgrades the card picked (a smith, an upgrade event).
const fn upgrade_select(decision: &RunDecision<'_>) -> bool {
    match decision {
        RunDecision::CardSelect(select) => matches!(
            select.purpose,
            SelectPurpose::Upgrade
                | SelectPurpose::Smith
                | SelectPurpose::Event {
                    text: PromptText::Upgrade,
                    ..
                }
        ),
        _ => false,
    }
}

/// The closing text of the event just finished ([`Run::finished_event`]): its title, then its
/// last page, until the next room; empty without one.
fn finished_event(run: &Run, describer: &Describer) -> Vec<Rich> {
    let Some(room) = run.finished_event() else {
        return Vec::new();
    };
    let s = run.state();
    let vars = room.vars(s);
    let event = EventText {
        vars: &vars,
        character: Some(s.character),
        extra: &[],
    };
    let Some(body) = room
        .finished
        .as_deref()
        .and_then(|key| describer.event_description(key, &event))
    else {
        return Vec::new();
    };
    let title = describer
        .event_title(room.kind.id())
        .unwrap_or_else(|| names::title_case(room.kind.id()));
    let mut lines = vec![Rich(vec![RichSpan {
        fx: None,
        role: Role::Plain,
        bold: true,
        text: title,
    }])];
    lines.extend(text::lines(&body, Role::Body));
    lines
}

/// The last page of the event just finished ([`Run::finished_event`]): its title and text,
/// which the game shows with one `PROCEED` option (`NEventRoom.Proceed` opens the map).
#[must_use]
pub fn finished_event_page(run: &Run, describer: &Describer) -> Option<(String, Vec<Rich>)> {
    let room = run.finished_event()?;
    let key = room.finished.as_deref()?;
    let s = run.state();
    let vars = room.vars(s);
    let event = EventText {
        vars: &vars,
        character: Some(s.character),
        extra: &[],
    };
    let body = describer.event_description(key, &event)?;
    let title = describer
        .event_title(room.kind.id())
        .unwrap_or_else(|| names::title_case(room.kind.id()));
    Some((title, text::lines(&body, Role::Plain)))
}

/// The lines under the reward screen's header: the game's is only `Loot!`, so the phone shows
/// none of ours; the terminal keeps its summary.
fn rewards_lines(run: &Run, describer: &Describer, left: usize) -> Vec<Rich> {
    let summary = (!describer.game_wording()).then(|| {
        plain(match &run.state().room {
            Room::Combat { encounter, .. } => {
                let name = names::title_case(encounter);
                // Encounter ids end in their kind: `Punch Construct Normal`.
                let name = name
                    .strip_suffix(" Normal")
                    .or_else(|| name.strip_suffix(" Weak"))
                    .or_else(|| name.strip_suffix(" Elite"))
                    .unwrap_or(&name);
                format!("Victory over {name}.")
            }
            _ if left == 0 => "Nothing left to take.".to_owned(),
            _ => format!("{left} reward(s) left."),
        })
    });
    summary
        .into_iter()
        .chain(
            run.ended_combat()
                .map(|c| crate::combat::escaped_thefts(c.state()))
                .unwrap_or_default()
                .into_iter()
                .map(plain),
        )
        .collect()
}

/// The screen's title and the lines above its choices.
fn describe(run: &Run, describer: &Describer, open: Option<usize>) -> (String, Vec<Rich>) {
    let s = run.state();
    match run.pending_decision() {
        RunDecision::Ancient(room) => {
            let id = room.kind.id().to_uppercase();
            let said = describer.ancient_lines(&id, s.character);
            if said.is_empty() || !describer.game_wording() {
                let name = names::title_case(room.kind.id());
                let line = plain(format!("{name} offers a blessing. Choose one."));
                return (name, vec![line]);
            }
            let name = describer
                .line("ancients", &format!("{id}.title"))
                .unwrap_or_else(|| names::title_case(room.kind.id()));
            let lines = said
                .iter()
                .flat_map(|l| text::lines(l, Role::Plain))
                .collect();
            (name, lines)
        }
        RunDecision::Event(room) => {
            let vars = room.vars(s);
            let event = EventText {
                vars: &vars,
                character: Some(s.character),
                extra: &[],
            };
            let body = room.page_description().map_or_else(
                || {
                    room.options
                        .iter()
                        .find_map(|o| describer.event_body(&o.text_key, &event))
                },
                |key| describer.event_description(key, &event),
            );
            let lines = body.map_or_else(
                || {
                    room.options
                        .iter()
                        .find_map(|o| names::event_page(&o.text_key))
                        .map(|_| plain("Choose an option."))
                        .into_iter()
                        .collect()
                },
                |body| text::lines(&body, Role::Plain),
            );
            let title = describer
                .event_title(room.kind.id())
                .unwrap_or_else(|| names::title_case(room.kind.id()));
            (title, lines)
        }
        RunDecision::Rewards(_) if open.is_some() => (
            "Choose a card".to_owned(),
            vec![plain("Pick one for your deck, or skip.")],
        ),
        RunDecision::Rewards(rewards) => (
            "Rewards".to_owned(),
            rewards_lines(run, describer, rewards.len()),
        ),
        RunDecision::CardSelect(select) => describe_select(run, describer, select),
        RunDecision::Bundles(_) => (
            "Bundles".to_owned(),
            vec![plain("Choose a bundle; its cards join your deck.")],
        ),
        RunDecision::RestSite(_) => (
            "Rest site".to_owned(),
            describer
                .line("rest_site_ui", "PROMPT")
                .map(|p| vec![plain(p)])
                .unwrap_or_default(),
        ),
        RunDecision::Treasure(_) => ("Treasure".to_owned(), Vec::new()),
        RunDecision::Shop(_) => ("Shop".to_owned(), Vec::new()),
        RunDecision::Unsupported(point) => {
            let what = match &s.room {
                Room::Event { id } => format!("event {id}"),
                _ => s
                    .room_type()
                    .map_or_else(|| point.name(), |r| r.name())
                    .to_owned(),
            };
            (
                "Not ported".to_owned(),
                vec![plain(format!("this content is not ported yet: {what}"))],
            )
        }
        _ => (String::new(), Vec::new()),
    }
}

/// A card selection's title (what asks: the event or the relic) and its one prompt line: the
/// game's prompt (else what the pick does and how many), the picks so far when there are
/// several, and `Esc cancels` when the engine lets the player cancel.
fn describe_select(run: &Run, describer: &Describer, select: &CardSelect) -> (String, Vec<Rich>) {
    let state = run.state();
    let prompt = select.prompt(state);
    let title = match (prompt, &state.room) {
        (SelectPrompt::Relic(id), _) => describer.relic_title(id),
        (_, Room::Event { id }) => describer
            .event_title(id)
            .unwrap_or_else(|| names::title_case(id)),
        _ => select_title(select.purpose).to_owned(),
    };
    let mut line = describer
        .select_prompt(prompt, select.min(), select.count)
        .map_or_else(|| select_prompt(select), |p| text::plain(&p));
    if select.count > 1 {
        let _ = write!(line, " · {}/{} picked", select.picked.len(), select.count);
    }
    if select.purpose.is_cancelable()
        && !matches!(select.purpose, SelectPurpose::ChooseACard { .. })
    {
        line.push_str(" · Esc cancels");
    }
    (title, vec![plain(line)])
}

/// The title of a selection the run itself opens (rest site, shop, a reward).
const fn select_title(purpose: SelectPurpose) -> &'static str {
    match purpose {
        SelectPurpose::Smith => "Smith",
        SelectPurpose::Cook => "Cook",
        SelectPurpose::ShopRemoval { .. } => "Card removal",
        SelectPurpose::ChooseACard { .. } => "Choose a card",
        _ => "Choose cards",
    }
}

/// What a card selection asks for (the effect, for a player without the game's text) and how
/// many cards: `Upgrade: pick 1`, `Remove: pick up to 2`.
fn select_prompt(select: &CardSelect) -> String {
    let what = match select.purpose {
        SelectPurpose::Upgrade | SelectPurpose::Smith => "Upgrade".to_owned(),
        SelectPurpose::Cook | SelectPurpose::Remove { damage: 0 } | SelectPurpose::Relic { .. } => {
            "Remove".to_owned()
        }
        SelectPurpose::Remove { damage } => format!("Remove (then lose {damage} HP)"),
        SelectPurpose::ShopRemoval { cost, .. } => format!("Remove for {cost}g"),
        SelectPurpose::Enchant { .. } | SelectPurpose::EnchantUpTo { .. } => "Enchant".to_owned(),
        SelectPurpose::Event { text, .. } => match text {
            PromptText::Remove => "Remove",
            PromptText::Transform => "Transform",
            PromptText::Upgrade => "Upgrade",
            PromptText::Enchant => "Enchant",
            _ => "Choose",
        }
        .to_owned(),
        SelectPurpose::TransformRandom | SelectPurpose::RelicUpTo { .. } => "Transform".to_owned(),
        SelectPurpose::TransformRandomUpgraded => "Transform and upgrade".to_owned(),
        SelectPurpose::AddToDeck | SelectPurpose::AddToDeckUpTo => "Add to your deck".to_owned(),
        SelectPurpose::ChooseACard { .. } => "Choose a card".to_owned(),
        SelectPurpose::Clone => "Duplicate".to_owned(),
    };
    if select.min() < select.count {
        format!("{what}: pick up to {}", select.count)
    } else {
        format!("{what}: pick {}", select.count)
    }
}

const fn span(role: Role, text: String) -> RichSpan {
    RichSpan {
        fx: None,
        role,
        bold: false,
        text,
    }
}

fn role(role: Role, text: &str) -> Rich {
    Rich(vec![span(role, text.to_owned())])
}

fn plain(text: impl Into<String>) -> Rich {
    Rich(vec![span(Role::Plain, text.into())])
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use sts2_core::Action;
    use sts2_core::Character;
    use sts2_core::run::{Act1, MapCoord, MapPointType, RunRelic, RunState, UnlockState};

    use super::*;
    use crate::model::Input;
    use crate::text::Localization;
    use crate::ui::{Effect, Ui};

    /// A run played on with the first legal action (fights with the first card, rewards left
    /// behind, the map toward `kind`) until `want` holds of its decision.
    fn advance(kind: MapPointType, want: impl Fn(&RunDecision<'_>) -> bool) -> (Run, Describer) {
        let (run, describer) = neow();
        advance_from(run, describer, kind, want, |_| ())
    }

    /// [`advance`] from a run already set up.
    fn advance_from(
        mut run: Run,
        describer: Describer,
        kind: MapPointType,
        want: impl Fn(&RunDecision<'_>) -> bool,
        on_map: impl Fn(&mut RunState),
    ) -> (Run, Describer) {
        for _ in 0..4_000 {
            if want(&run.pending_decision()) {
                return (run, describer);
            }
            let legal = run.legal_actions();
            let action = match run.pending_decision() {
                RunDecision::Map(nodes) => {
                    let near = |c| distance_to(&run, c, kind);
                    nodes
                        .iter()
                        .copied()
                        .min_by_key(|&c| near(c))
                        .map(RunAction::ChooseNode)
                }
                RunDecision::Combat(_) => legal
                    .iter()
                    .find(|a| matches!(a, RunAction::Combat(Action::PlayCard { .. })))
                    .or_else(|| legal.iter().find(|a| matches!(a, RunAction::Combat(_))))
                    .cloned(),
                _ => legal
                    .iter()
                    .rev()
                    .find(|a| **a == RunAction::Proceed)
                    .cloned()
                    .or_else(|| legal.first().cloned()),
            };
            let action = action.expect("a way on");
            if matches!(run.pending_decision(), RunDecision::Map(_)) {
                let state = run.loadout_state().expect("on the map");
                state.hp = 999;
                on_map(state);
            }
            run.apply(action).expect("legal");
        }
        panic!("the room was not reached");
    }

    /// Floors from `from` to the nearest point of `kind` above it.
    fn distance_to(run: &Run, from: MapCoord, kind: MapPointType) -> usize {
        let map = &run.state().map;
        let mut layer = vec![from];
        for depth in 0..20 {
            if layer
                .iter()
                .any(|&c| map.point(c).is_some_and(|p| p.kind == kind))
            {
                return depth;
            }
            layer = layer
                .iter()
                .filter_map(|&c| map.point(c))
                .flat_map(|p| p.children.iter().collect::<Vec<_>>())
                .collect();
        }
        usize::MAX
    }

    fn neow() -> (Run, Describer) {
        let run = Run::new(
            Character::Ironclad,
            "12345",
            0,
            UnlockState::all(),
            Act1::Random,
        )
        .expect("run")
        .with_events();
        let describer = Describer::new(Rc::new(Localization::empty()));
        (run, describer)
    }

    #[test]
    fn an_upgrade_row_shows_the_upgraded_cost_beside_the_cost_now() {
        let describer = Describer::new(Rc::new(Localization::empty()));
        // Body Slam: `EnergyCost.UpgradeBy(-1)`.
        let face = upgraded_face(&describer, RunCard::new("BODY_SLAM"));
        let now = face.before.as_deref().expect("upgradable");
        assert_eq!(
            (face.name.as_str(), face.cost.as_str()),
            ("Body Slam+", "0")
        );
        assert_eq!((now.name.as_str(), now.cost.as_str()), ("Body Slam", "1"));
        // An X cost stays X.
        let face = upgraded_face(&describer, RunCard::new("WHIRLWIND"));
        let now = face.before.as_deref().expect("upgradable");
        assert_eq!((face.cost.as_str(), now.cost.as_str()), ("X", "X"));
        // An unplayable status cannot be upgraded: the card as it is, no second face.
        let face = upgraded_face(&describer, RunCard::new("WOUND"));
        assert_eq!((face.cost.as_str(), face.before.is_none()), ("—", true));
    }

    #[test]
    fn neow_is_a_titled_list_of_numbered_rows() {
        let (run, describer) = neow();
        let screen = screen(&run, &describer, &View::default()).expect("a room screen");
        assert_eq!(
            screen.title.as_ref().map(Rich::text).as_deref(),
            Some("Neow")
        );
        let Some(Block::List { rows, page, .. }) = screen.blocks.last() else {
            panic!("a list");
        };
        assert!(page.is_none() && screen.primary.is_none());
        let ids: Vec<u32> = rows.iter().map(|r| r.id.0).collect();
        assert_eq!(
            ids,
            (1..=u32::try_from(rows.len()).expect("few")).collect::<Vec<_>>()
        );
        assert_eq!(rows[0].key, Some(Key::Digit(1)));
        assert!(rows.iter().all(|r| r.state == ButtonState::Enabled));
    }

    #[test]
    fn a_tap_or_a_digit_picks_the_row_it_names() {
        let (run, describer) = neow();
        let mut ui = Ui::default();
        let effects = ui.room_input(&run, &describer, 0, &Input::Tap(ButtonId(2)));
        assert_eq!(effects, vec![Effect::Apply(RunAction::AncientOption(1))]);
        let effects = ui.room_input(&run, &describer, 0, &Input::Key(Key::Digit(3)));
        assert_eq!(effects, vec![Effect::Apply(RunAction::AncientOption(2))]);
        let effects = ui.room_input(&run, &describer, 0, &Input::Key(Key::Digit(9)));
        assert_eq!(effects, vec![Effect::Error("no option 9".to_owned())]);
    }

    #[test]
    fn a_screen_is_a_function_of_the_run_and_the_view() {
        let (run, describer) = neow();
        let a = screen(&run, &describer, &View::default());
        let b = screen(&run, &describer, &View::default());
        assert_eq!(a, b);
        let json = serde_json::to_string(&a).expect("serialize");
        assert_eq!(a, serde_json::from_str(&json).expect("deserialize"));
    }

    #[test]
    fn paging_keys_are_digits_with_the_main_button_zero() {
        let item = |action, enabled| MenuItem {
            label: String::new(),
            detail: None,
            action,
            enabled,
            warning: None,
            open: None,
            section: None,
            copies: 1,
        };
        let mut items: Vec<MenuItem> = (0..12)
            .map(|i| item(RunAction::SelectCard(i), true))
            .collect();
        items.push(item(RunAction::ConfirmSelect, true));
        let paging = Paging::of(&items);
        assert!(paging.paged());
        assert_eq!(paging.pages(), 2);
        assert_eq!(paging.key(13), "0");
        assert_eq!(paging.key(10), "1");
        assert_eq!(paging.pick(1, 2), Some(11));
    }

    fn on_rewards() -> (Run, Describer) {
        advance(MapPointType::Monster, |d| {
            matches!(d, RunDecision::Rewards(_))
        })
    }

    /// Prayer Wheel's extra card reward is a second `Card reward` row naming the relic; the
    /// first, and a run without the relic, stay plain (the engine adds the row, it is no repeat).
    #[test]
    fn a_relics_extra_card_reward_is_labelled_with_it() {
        let labels = |run: &Run, describer: &Describer| -> Vec<String> {
            list(&screen(run, describer, &View::default()).expect("screen"))
                .iter()
                .filter_map(|r| {
                    let text = r.label.text();
                    text.starts_with("Card reward").then_some(text)
                })
                .collect()
        };
        let want = |d: &RunDecision<'_>| matches!(d, RunDecision::Rewards(_));
        let (run, describer) = on_rewards();
        assert_eq!(labels(&run, &describer), ["Card reward"]);
        let (run, describer) = neow();
        let wheel = |s: &mut RunState| {
            if !s.relics.iter().any(|r| r.kind == RelicKind::PrayerWheel) {
                s.relics.push(RunRelic::new(RelicKind::PrayerWheel));
            }
        };
        let (run, describer) = advance_from(run, describer, MapPointType::Monster, want, wheel);
        assert_eq!(
            labels(&run, &describer),
            ["Card reward", "Card reward (Prayer Wheel)"]
        );
    }

    fn list(screen: &Screen) -> &[Button] {
        match screen.blocks.last() {
            Some(Block::List { rows, .. }) => rows,
            _ => panic!("a list"),
        }
    }

    #[test]
    fn rewards_are_rows_with_leave_as_the_main_button() {
        let (run, describer) = on_rewards();
        let screen = screen(&run, &describer, &View::default()).expect("a room screen");
        assert_eq!(
            screen.title.as_ref().map(Rich::text).as_deref(),
            Some("Rewards")
        );
        let rows = list(&screen);
        assert_ne!(rows.len(), 0);
        let main = screen.primary.as_ref().expect("Leave");
        assert_eq!(main.label.text(), "Leave");
        assert_eq!(usize::try_from(main.id.0).expect("small"), rows.len() + 1);
    }

    #[test]
    fn a_reward_screen_leaves_on_its_own_once_nothing_is_left() {
        let (mut run, _) = on_rewards();
        loop {
            let RunDecision::Rewards(left) = run.pending_decision() else {
                panic!("still on the rewards");
            };
            let action = match left.first() {
                Some(Reward::Card(_)) => RunAction::SkipReward(0),
                Some(_) => RunAction::ClaimReward(0),
                None => break,
            };
            assert_eq!(crate::ui::follow_up(&run), None, "rewards are left");
            run.apply(action).expect("legal");
        }
        assert_eq!(crate::ui::follow_up(&run), Some(RunAction::Proceed));
    }

    #[test]
    fn a_card_reward_opens_as_its_own_screen_and_esc_closes_it() {
        let (run, describer) = on_rewards();
        let mut ui = Ui::default();
        let items = ui.room_items(&run, &describer);
        let n = items
            .iter()
            .filter(|i| i.enabled)
            .position(|i| i.open.is_some())
            .expect("a card reward")
            + 1;
        let effects = ui.room_input(
            &run,
            &describer,
            0,
            &Input::Key(Key::Digit(u8::try_from(n).expect("a digit"))),
        );
        assert_eq!(effects, vec![]);
        let open = ui.open_reward(&run).expect("opened");
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        assert_eq!(
            screen.title.as_ref().map(Rich::text).as_deref(),
            Some("Choose a card")
        );
        assert!(list(&screen).iter().any(|r| r.card.is_some()));
        assert!(list(&screen).iter().any(|r| r.key == Some(Key::Char('s'))));
        assert_eq!(ui.room_input(&run, &describer, 0, &Input::Back), vec![]);
        assert_eq!(ui.open_reward(&run), None, "closed (reward {open})");
    }

    #[test]
    fn leaving_rewards_behind_asks_in_place_and_the_second_press_takes_it() {
        let (run, describer) = on_rewards();
        let mut ui = Ui::default();
        let leave = ui
            .room_screen(&run, &describer, 0)
            .expect("screen")
            .primary
            .expect("Leave");
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Tap(leave.id)),
            vec![]
        );
        let asking = ui.asking().expect("asks first").clone();
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        let main = screen.primary.expect("Leave");
        assert_eq!(main.state, ButtonState::Picked);
        assert_eq!(main.label.text(), asking.label());
        // Another input puts it back; the same tap takes it.
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Key(Key::Char('x'))),
            vec![]
        );
        assert!(ui.asking().is_none());
        ui.room_input(&run, &describer, 0, &Input::Tap(leave.id));
        let effects = ui.room_input(&run, &describer, 0, &Input::Tap(leave.id));
        assert_eq!(effects, vec![Effect::Apply(RunAction::Proceed)]);
    }

    #[test]
    fn the_confirmations_setting_lets_leaving_through() {
        let (run, describer) = on_rewards();
        let mut ui = Ui::default();
        ui.set_confirmations(false);
        let leave = ui
            .room_screen(&run, &describer, 0)
            .expect("screen")
            .primary
            .expect("Leave");
        let effects = ui.room_input(&run, &describer, 0, &Input::Tap(leave.id));
        assert_eq!(effects, vec![Effect::Apply(RunAction::Proceed)]);
    }

    fn on_shop() -> (Run, Describer) {
        advance(MapPointType::Shop, |d| matches!(d, RunDecision::Shop(_)))
    }

    fn row_texts(screen: &Screen) -> Vec<(String, Option<String>)> {
        list(screen)
            .iter()
            .map(|r| (r.label.text(), r.right.as_ref().map(Rich::text)))
            .collect()
    }

    #[test]
    fn the_shop_opens_on_its_sections_with_counts_then_remove_and_leave() {
        let (run, describer) = on_shop();
        let ui = Ui::default();
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        let rows = row_texts(&screen);
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Cards",
                "Colorless cards",
                "Relics",
                "Potions",
                "Remove a card"
            ]
        );
        for (section, (_, right)) in ShopSection::ALL.iter().zip(&rows) {
            let n = menu::section_count(&run, *section);
            assert_eq!(right.as_deref(), Some(section.count_label(n).as_str()));
        }
        assert_eq!(
            screen.primary.expect("Leave").label.text(),
            "Leave the shop"
        );
    }

    #[test]
    fn a_section_opens_lists_its_items_and_esc_returns() {
        let (run, describer) = on_shop();
        let mut ui = Ui::default();
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Key(Key::Digit(1))),
            vec![]
        );
        assert_eq!(ui.open_section(&run), Some(ShopSection::Cards));
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        assert_eq!(
            screen.title.as_ref().map(Rich::text).as_deref(),
            Some("Cards")
        );
        let n = menu::section_count(&run, ShopSection::Cards);
        assert_eq!(list(&screen).len(), n);
        assert!(
            list(&screen)
                .iter()
                .all(|r| r.card.is_some() && r.right.is_some())
        );
        let back = screen.primary.as_ref().expect("a Back button");
        assert_eq!((back.id, back.key), (BACK, Some(Key::Esc)));
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Tap(BACK)),
            vec![]
        );
        assert_eq!(ui.open_section(&run), None);
        // At the first level Esc leaves, as before.
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Back),
            vec![Effect::Apply(RunAction::Proceed)]
        );
    }

    #[test]
    fn a_flat_shop_lists_every_item_on_one_level_and_a_tap_asks_then_buys() {
        let (run, describer) = on_shop();
        let mut ui = Ui::default();
        ui.set_flat_shop(true);
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        let rows = list(&screen);
        let on_sale: usize = ShopSection::ALL
            .iter()
            .map(|s| menu::section_count(&run, *s))
            .sum();
        // Every item on sale, then Remove a card; Leave is the primary button.
        assert_eq!(rows.len(), on_sale + 1);
        assert!(rows.iter().take(on_sale).all(|r| r.right.is_some()));
        assert_eq!(
            screen.primary.expect("Leave").label.text(),
            "Leave the shop"
        );
        let items = ui.room_items(&run, &describer);
        let Some(at) = items
            .iter()
            .filter(|i| i.enabled)
            .position(|i| matches!(i.action, RunAction::BuyCard(_)))
        else {
            return; // no card affordable in this seed's shop
        };
        let action = items
            .iter()
            .filter(|i| i.enabled)
            .nth(at)
            .map(|i| i.action.clone());
        let tap = Input::Tap(ButtonId(u32::try_from(at + 1).expect("an id")));
        assert_eq!(ui.room_input(&run, &describer, 0, &tap), vec![]);
        assert_eq!(ui.open_section(&run), None);
        assert!(ui.asking().is_some_and(|a| a.what.starts_with("Buy for")));
        assert_eq!(
            ui.room_input(&run, &describer, 0, &tap),
            vec![Effect::Apply(action.expect("the card's action"))]
        );
    }

    #[test]
    fn a_shop_section_with_nothing_to_buy_says_why() {
        // Broke, with a full belt: no potion can be bought, and the page says why.
        let (run, describer) = neow();
        let (run, describer) = advance_from(
            run,
            describer,
            MapPointType::Shop,
            |d| matches!(d, RunDecision::Shop(_)),
            |state| {
                state.gold = 0;
                let slots = state.potions.len();
                state.potions = vec![Some("BLOCK_POTION"); slots];
            },
        );
        let mut ui = Ui::default();
        let potions = ShopSection::ALL
            .iter()
            .position(|s| *s == ShopSection::Potions)
            .expect("a potions section")
            + 1;
        ui.room_input(
            &run,
            &describer,
            0,
            &Input::Key(Key::Digit(u8::try_from(potions).expect("a digit"))),
        );
        assert_eq!(ui.open_section(&run), Some(ShopSection::Potions));
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        let said: Vec<String> = screen
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Text(lines) => Some(lines.iter().map(Rich::text)),
                _ => None,
            })
            .flatten()
            .collect();
        assert!(
            said.iter().any(|l| l.contains("belt is full")),
            "the page says why: {said:?}"
        );
    }

    #[test]
    fn buying_in_a_section_asks_in_place_and_the_second_press_buys() {
        let (run, describer) = on_shop();
        let mut ui = Ui::default();
        ui.room_input(&run, &describer, 0, &Input::Key(Key::Digit(1)));
        let items = ui.room_items(&run, &describer);
        let Some(first) = items.iter().find(|i| i.enabled) else {
            return; // nothing affordable in this seed's shop
        };
        let action = first.action.clone();
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Key(Key::Digit(1))),
            vec![]
        );
        let asking = ui.asking().expect("asks first");
        assert!(asking.what.starts_with("Buy for"));
        let screen = ui.room_screen(&run, &describer, 0).expect("screen");
        assert!(
            row_texts(&screen)
                .iter()
                .any(|(l, _)| l.starts_with("Buy for"))
        );
        assert_eq!(
            ui.room_input(&run, &describer, 0, &Input::Key(Key::Digit(1))),
            vec![Effect::Apply(action)]
        );
    }

    #[test]
    fn remove_a_card_is_a_direct_row_that_asks_in_place() {
        let (run, describer) = on_shop();
        let mut ui = Ui::default();
        let items = ui.room_items(&run, &describer);
        let Some(at) = items
            .iter()
            .filter(|i| i.enabled)
            .position(|i| i.action == RunAction::RemoveCard)
        else {
            return; // too poor to remove a card
        };
        ui.room_input(
            &run,
            &describer,
            0,
            &Input::Tap(ButtonId(u32::try_from(at + 1).expect("small"))),
        );
        assert!(
            ui.asking()
                .is_some_and(|a| a.what.starts_with("Remove a card"))
        );
    }

    #[test]
    fn the_end_screen_exists_only_at_the_end_and_its_buttons_print_their_keys() {
        let (run, describer) = neow();
        assert_eq!(end_screen(&run, &describer, "12345", &[]), None);
        let ui = Ui::default();
        assert_eq!(
            ui.end_input(&Input::Tap(ButtonId(1))),
            vec![Effect::End(EndAction::Retry)]
        );
        assert_eq!(
            ui.end_input(&Input::Key(Key::Char('q'))),
            vec![Effect::End(EndAction::Quit)]
        );
        assert_eq!(
            ui.end_input(&Input::Key(Key::Char('x'))),
            vec![Effect::Pass]
        );
    }
}
