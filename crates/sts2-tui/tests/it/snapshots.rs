//! Snapshots of the run screens (a fight, a fight waiting for a target, a fight's card choice,
//! the card reward, the map, the shop, the end of the run and the character select) at the
//! sizes the design targets: a phone over SSH (45×30), a classic
//! 80×24, a laptop (120×35) and large terminals (160×45, 200×55). One fixed seed, a small made-up text
//! table, so the screens do not depend on what is installed.
//!
//! A changed screen fails with the new render written beside the old one (`*.new`); run with
//! `UPDATE_SNAPSHOTS=1` to accept the new ones.

use std::path::PathBuf;
use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::KeyCode;
use sts2_core::combat::DecisionRequest;
use sts2_core::run::{
    Act1, MapCoord, MapPointType, Run, RunAction, RunCard, RunDecision, UnlockState,
};
use sts2_core::{Character, Combat};
use sts2_tui::app::{App, Input};
use sts2_tui::hit::{Target, UiAction};
use sts2_tui::shell::Shell;
use sts2_tui::text::Localization;
use sts2_tui::ui;
use sts2_view::model::Screen;

pub const SEED: &str = "12345";
const SIZES: [(u16, u16); 5] = [(45, 30), (80, 24), (120, 35), (160, 45), (200, 55)];

/// Made-up card and potion text in the game's markup, with the game's variable names.
pub fn text() -> Rc<Localization> {
    Rc::new(Localization::from_json(&[
        (
            "cards",
            r#"{"STRIKE_IRONCLAD.description": "Deal {Damage:diff()} damage.",
                "DEFEND_IRONCLAD.description": "Gain {Block:diff()} [gold]Block[/gold].",
                "BASH.description": "Deal {Damage:diff()} damage.\nApply {VulnerablePower:diff()} [gold]Vulnerable[/gold]."}"#,
        ),
        (
            "potions",
            r#"{"FIRE_POTION.description": "Deal [blue]{Damage}[/blue] damage to an enemy.",
                "BLOCK_POTION.description": "Gain [blue]{Block}[/blue] [gold]Block[/gold]."}"#,
        ),
        (
            "relics",
            r#"{"TOASTY_MITTENS.title": "Toasty Mittens",
                "BURNING_BLOOD.description": "At the end of combat, heal [blue]6[/blue] HP."}"#,
        ),
        (
            "card_selection",
            r#"{"TO_EXHAUST": "Choose {Amount:plural:a card|[blue]{}[/blue] cards} to [gold]Exhaust[/gold]."}"#,
        ),
        (
            "characters",
            r#"{"IRONCLAD.description": "The remaining soldier of the Ironclads. Sold his soul to harness demonic energies."}"#,
        ),
    ]))
}

/// [`map_run`] with the deck replaced by `deck` (card ids).
fn map_run_with_deck(hp: i32, deck: &[&'static str]) -> Run {
    let mut run = map_run(hp);
    run.loadout_state().expect("on the map").deck =
        deck.iter().map(|id| RunCard::new(id)).collect();
    run
}

/// The first fight with `deck` (card ids) for the deck.
pub fn fight_with_deck(deck: &[&'static str]) -> App {
    let mut app = App::with_run_text(SEED, 0, map_run_with_deck(80, deck), text());
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    assert!(app.combat().is_some(), "the first floor is a fight");
    app
}

/// The run after Neow, on the map, with two potions on the belt and `hp`.
pub fn map_run(hp: i32) -> Run {
    map_run_with_potions(hp, hp, &[Some("FIRE_POTION"), Some("BLOCK_POTION"), None])
}

/// [`map_run`] with `hp` of `max_hp` and `potions` on the belt.
pub fn map_run_with_potions(hp: i32, max_hp: i32, potions: &[Option<&'static str>]) -> Run {
    map_run_as(Character::Ironclad, hp, max_hp, potions)
}

/// [`map_run_with_potions`] as `character`.
fn map_run_as(character: Character, hp: i32, max_hp: i32, potions: &[Option<&'static str>]) -> Run {
    let mut run = Run::new(character, SEED, 0, UnlockState::all(), Act1::Random)
        .expect("run")
        .with_events();
    for _ in 0..50 {
        if matches!(run.pending_decision(), RunDecision::Map(_)) {
            break;
        }
        let next = run
            .legal_actions()
            .into_iter()
            .find(|a| run.clone().apply(a.clone()).is_ok())
            .expect("a way on");
        run.apply(next).expect("legal");
    }
    let state = run.loadout_state().expect("on the map");
    state.gold = 300;
    state.hp = hp;
    state.max_hp = max_hp;
    state.potions = potions.to_vec();
    run
}

pub fn map_app() -> App {
    App::with_run_text(SEED, 0, map_run(80), text())
}

/// [`map_app`] with `potions` on the belt and 60 of 80 HP.
pub fn map_app_with_potions(potions: &[Option<&'static str>]) -> App {
    App::with_run_text(SEED, 0, map_run_with_potions(60, 80, potions), text())
}

pub fn reachable(app: &App) -> Vec<MapCoord> {
    app.run()
        .legal_actions()
        .into_iter()
        .filter_map(|a| match a {
            RunAction::ChooseNode(c) => Some(c),
            _ => None,
        })
        .collect()
}

pub fn press(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.handle_key(KeyCode::Char(c));
    }
}

/// The first fight.
pub fn fight() -> App {
    let mut app = map_app();
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    assert!(app.combat().is_some(), "the first floor is a fight");
    app
}

/// The first fight at its first turn's card choice: Toasty Mittens asks for a card to exhaust.
fn choice() -> App {
    let mut run = map_run(80);
    run.loadout_relic("TOASTY_MITTENS").expect("ported");
    let mut app = App::with_run_text(SEED, 0, run, text());
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    assert!(
        app.combat()
            .is_some_and(|c| c.status() == sts2_core::Status::Decision),
        "Toasty Mittens asks at the first turn"
    );
    app
}

/// The run lost in its first fight: the end screen.
fn defeat() -> App {
    let mut app = App::with_run_text(SEED, 0, map_run(1), text());
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    for _ in 0..20 {
        if matches!(app.run().pending_decision(), RunDecision::GameOver) {
            return app;
        }
        press(&mut app, "e");
    }
    panic!("the run did not end");
}

/// The character select, as the game opens.
fn select() -> Shell {
    Shell::select(text())
}

fn render_shell(shell: &Shell, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
    terminal.draw(|f| ui::draw_shell(f, shell)).expect("draw");
    text_of(terminal.backend().buffer(), width)
}

/// The first fight with an attack selected and waiting for its target.
fn targeting() -> App {
    let mut app = fight();
    let hand = app.combat().expect("fight").state().player.hand.len();
    for n in 1..=hand {
        let mut copy = app.clone();
        press(&mut copy, &n.to_string());
        if matches!(copy.input(), Input::Target(_)) {
            app = copy;
            break;
        }
    }
    assert!(matches!(app.input(), Input::Target(_)), "an attack waits");
    app
}

/// The cards picked so far in the fight's card choice, if one is open.
fn picked_count(combat: &Combat) -> Option<usize> {
    let DecisionRequest::ChooseCards { picked, .. } = combat.pending_decision()?;
    Some(picked.len())
}

/// Wins the fight in progress with the keys: the first card on the first enemy, then the end
/// of the turn. A card choice lists its picked cards (a number toggles one) with Done last, so
/// the script presses the number that closes the choice.
pub fn win(app: &mut App) {
    for key in "111e".chars().cycle().take(1200) {
        if app.combat().is_none() {
            return;
        }
        // A card choice keeps its picked cards in the list (a number toggles one) with Done
        // last, so "1" no longer walks it: a key takes the first number that picks a card still
        // on offer, or closes the choice once the picks are full.
        if let Some(picked) = app.combat().and_then(picked_count) {
            let next = (1..=9).find(|n| {
                let mut copy = app.clone();
                press(&mut copy, &n.to_string());
                copy.combat()
                    .is_none_or(|c| picked_count(c).is_none_or(|p| p > picked))
            });
            if let Some(n) = next {
                press(app, &n.to_string());
                continue;
            }
        }
        app.handle_key(KeyCode::Char(key));
    }
    panic!("the fight did not end");
}

/// The first fight won with the keys, on its rewards.
pub fn rewards() -> App {
    let mut app = fight();
    win(&mut app);
    assert!(matches!(
        app.run().pending_decision(),
        RunDecision::Rewards(_)
    ));
    app
}

/// The first fight's card reward open as its own screen.
fn card_reward() -> App {
    let mut app = rewards();
    let row = app
        .menu()
        .iter()
        .filter(|i| i.enabled)
        .position(|i| i.open.is_some())
        .expect("a card reward");
    press(&mut app, &(row + 1).to_string());
    assert!(app.open_reward().is_some());
    app
}

/// Back on the map after the first fight, its rewards left behind.
fn map() -> App {
    let mut app = rewards();
    let n = app.menu().iter().filter(|i| i.enabled).count();
    app.perform(UiAction::Pick(n));
    app.perform(UiAction::Yes);
    assert!(matches!(app.run().pending_decision(), RunDecision::Map(_)));
    app
}

/// The first shop of the act, reached by the shortest path (fights won by the keys with HP to
/// spare, rewards left behind).
pub fn shop() -> App {
    let mut app = App::with_run_text(SEED, 0, map_run(999), text());
    for _ in 0..40 {
        if matches!(app.run().pending_decision(), RunDecision::Shop(_)) {
            return app;
        }
        match app.run().pending_decision() {
            RunDecision::Map(nodes) => {
                let next = nodes
                    .iter()
                    .copied()
                    .min_by_key(|&c| distance_to_shop(app.run(), c))
                    .expect("a node");
                app.perform(UiAction::Travel(next));
            }
            RunDecision::Combat(_) => win(&mut app),
            _ => {
                let n = app.menu().iter().filter(|i| i.enabled).count();
                app.perform(UiAction::Pick(n));
                app.perform(UiAction::Yes);
            }
        }
    }
    panic!("no shop reached");
}

/// Floors from `from` to the nearest shop above it.
fn distance_to_shop(run: &Run, from: MapCoord) -> usize {
    let map = &run.state().map;
    let mut layer = vec![from];
    for depth in 0..20 {
        if layer
            .iter()
            .any(|&c| map.point(c).is_some_and(|p| p.kind == MapPointType::Shop))
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

pub fn render(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
    terminal.draw(|f| ui::draw(f, app)).expect("draw");
    text_of(terminal.backend().buffer(), width)
}

/// The rows of `buffer`, each trimmed at the right.
pub fn text_of(buffer: &ratatui::buffer::Buffer, width: u16) -> String {
    buffer
        .content()
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
                + "\n"
        })
        .collect()
}

pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// Compares `app`'s screen at every size with its snapshot.
fn check(name: &str, app: &App) {
    check_drawn(name, |w, h| render(app, w, h));
}

/// Compares the screen `draw` renders at every size with its snapshot.
fn check_drawn(name: &str, draw: impl Fn(u16, u16) -> String) {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let mut failed = Vec::new();
    for (width, height) in SIZES {
        let file = dir().join(format!("{name}-{width}x{height}.txt"));
        let drawn = draw(width, height);
        if update {
            std::fs::create_dir_all(dir()).expect("snapshot dir");
            std::fs::write(&file, &drawn).expect("write snapshot");
            continue;
        }
        let old = std::fs::read_to_string(&file).unwrap_or_default();
        if old != drawn {
            let new = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(file.file_name().expect("name"))
                .with_extension("new");
            std::fs::write(&new, &drawn).expect("write the new render");
            failed.push(format!("{} (new: {})", file.display(), new.display()));
        }
    }
    assert!(
        failed.is_empty(),
        "changed screens (UPDATE_SNAPSHOTS=1 accepts them):\n{}",
        failed.join("\n")
    );
}

/// The first fight with the `≡` menu open.
fn menu_open() -> App {
    let mut app = fight();
    app.handle_key(KeyCode::Tab);
    app
}

/// The first fight with the Block Potion selected: its row asks in place.
fn potion_selected() -> App {
    let mut app = fight();
    press(&mut app, "c");
    assert_eq!(app.input(), &Input::Potion(1));
    app
}

#[test]
fn combat_screens_match_their_snapshots() {
    check("combat", &fight());
    check("combat-target", &targeting());
    check("combat-choice", &choice());
    check("combat-potion", &potion_selected());
    check("menu", &menu_open());
}

/// A big screen draws larger: two-row HP bars, and from 160×45 each card takes two rows, its
/// text under its name. The energy is one plain line at every size. A small screen draws as
/// designed: one-row bars and cards.
#[test]
fn big_screens_draw_thick_bars_and_tall_cards() {
    let app = fight();
    // The second rows of thick bars: the bars on rows with no numbers (enemies side by side
    // put several on one row).
    let bars = |screen: &str| {
        screen
            .lines()
            .filter(|l| l.contains("████") && !l.contains('/'))
            .map(|l| l.matches('▕').count().max(1))
            .sum::<usize>()
    };
    for (width, height) in [(120, 35), (160, 45), (200, 55)] {
        let screen = render(&app, width, height);
        assert!(
            bars(&screen) >= 4,
            "{width}x{height}: thick bars (a second row of bar alone)\n{screen}"
        );
        assert!(
            screen.contains("⚔ 3 damage"),
            "{width}x{height}: the intent says what its number is\n{screen}"
        );
    }
    // A big screen's hand is card tiles (user, 2026-10-30): key, cost and name, then the text.
    for (width, height) in [(160, 45), (200, 55)] {
        let screen = render(&app, width, height);
        assert!(
            screen.contains("│▸1  ϟ1 Strike") && screen.contains("│Deal 6 damage."),
            "{width}x{height}: card tiles\n{screen}"
        );
    }
    for (width, height) in [(45, 30), (80, 24), (99, 40), (120, 34)] {
        let screen = render(&app, width, height);
        assert!(bars(&screen) == 0, "{width}x{height}: thin bars\n{screen}");
    }
    for (width, height) in SIZES {
        let screen = render(&app, width, height);
        assert_eq!(
            screen.matches("ϟ 3/3 Energy").count(),
            1,
            "{width}x{height}: the energy once\n{screen}"
        );
    }
}

/// The column takes most of a big screen and keeps to three fifths below it.
#[test]
fn the_column_fills_a_big_screen() {
    let app = fight();
    let bar_rule = |screen: &str| {
        screen
            .lines()
            .find(|l| l.contains("────"))
            .map_or(0, |l| l.trim().chars().count())
    };
    assert_eq!(bar_rule(&render(&app, 45, 30)), 43);
    assert_eq!(
        bar_rule(&render(&app, 99, 35)),
        57,
        "three fifths of 99, less the margins"
    );
    assert_eq!(
        bar_rule(&render(&app, 120, 35)),
        103,
        "seven eighths of 120, less the margins"
    );
    assert_eq!(bar_rule(&render(&app, 160, 45)), 138);
    assert_eq!(bar_rule(&render(&app, 200, 55)), 150, "the widest");
}

#[test]
fn the_end_and_select_screens_match_their_snapshots() {
    check("end", &defeat());
    let select = select();
    check_drawn("select", |w, h| render_shell(&select, w, h));
}

/// The end screen names the enemy that struck the last blow and its move, not the whole
/// encounter, and counts the rooms.
#[test]
fn the_end_screen_names_the_killer() {
    let app = defeat();
    let screen = render(&app, 80, 30);
    assert!(screen.contains("DEFEAT"), "{screen}");
    assert!(
        screen.contains("Slain by Leaf Slime (") || screen.contains("Slain by Twig Slime ("),
        "the killer and its move\n{screen}"
    );
    assert!(
        !screen.contains("Slain by Leaf Slime (S), "),
        "not the encounter\n{screen}"
    );
    assert!(screen.contains("1 fight"), "{screen}");
    assert!(screen.contains("Rooms visited"), "{screen}");
}

/// A card choice of the fight puts its prompt on the hand frame's title row; one that a play
/// opened keeps the incoming line as that play began, dim.
#[test]
fn a_card_choice_keeps_the_incoming_line() {
    let screen = render(&choice(), 120, 35);
    assert!(
        screen.contains("Toasty Mittens · Choose a card to Exhaust (0 of 1)"),
        "{screen}"
    );
    // The first of these the engine ports opens a choice when played.
    let played = ["ARMAMENTS", "WARCRY", "TRUE_GRIT"]
        .into_iter()
        .find_map(|card| {
            let mut app = fight_with_deck(&[card; 6]);
            press(&mut app, "1");
            app.combat()
                .is_some_and(|c| c.status() == sts2_core::Status::Decision)
                .then_some(app)
        })
        .expect("a card that opens a choice");
    let screen = render(&played, 120, 35);
    assert!(screen.contains("⚔ −7 HP"), "{screen}");
    assert!(screen.contains("(before this play)"), "{screen}");
}

/// No height hides a card or End Turn: the hand scrolls to the focused card and End Turn
/// stays, however few rows there are.
#[test]
fn no_height_hides_the_focused_card_or_end_turn() {
    let deck = ["BASH", "BASH", "BASH", "BASH", "BASH", "BASH"];
    let mut app = fight_with_deck(&deck);
    let hand = app.combat().expect("fight").state().player.hand.len();
    for n in 0..hand {
        if n > 0 {
            app.handle_key(KeyCode::Down);
        }
        for height in [9, 10, 11, 12, 14, 16, 18, 20] {
            let screen = render(&app, 45, height);
            assert!(
                screen.contains(&format!("▸{}", n + 1)),
                "45x{height}: card {} on screen\n{screen}",
                n + 1
            );
            assert!(screen.contains("End Turn"), "45x{height}:\n{screen}");
        }
    }
}

/// What comes at you is on the fight screen at every size: the incoming line under your HP,
/// and each attack's damage in its intent.
#[test]
fn the_incoming_line_and_the_intents_show_at_every_size() {
    let app = fight();
    for (width, height) in SIZES.into_iter().chain([(60, 20), (40, 14)]) {
        let screen = render(&app, width, height);
        assert!(screen.contains("⚔ −7 HP"), "{width}x{height}:\n{screen}");
        assert!(
            screen.contains("⚔ 3"),
            "{width}x{height}: the first intent\n{screen}"
        );
    }
}

/// Where the parts of a creature line that are always there sit: for each row with an HP bar,
/// the row, the column of the bar's first cell and the column of its `/`; for each row that
/// names the first enemy, the column of its name.
fn anchors(screen: &str) -> Vec<(usize, usize, usize)> {
    screen
        .lines()
        .enumerate()
        .filter_map(|(row, line)| {
            let bar = line.chars().position(|c| "█▒░".contains(c));
            let slash = line.chars().position(|c| c == '/');
            match (bar, slash) {
                (Some(bar), Some(slash)) => Some((row, bar, slash)),
                _ => line
                    .find("Leaf Slime (S)")
                    .map(|at| (row, line[..at].chars().count(), 0)),
            }
        })
        .collect()
}

/// Whether the screen draws a card's preview into an HP bar (the HP it would take, in the
/// preview's colour).
fn previewed(app: &App, width: u16, height: u16) -> bool {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
    terminal.draw(|f| ui::draw(f, app)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|c| c.symbol() == "█" && c.fg == ui::theme::LOSS)
}

/// Moving the focus changes what the enemies' lines show, never where they are: the bars,
/// the HP numbers and the names keep their columns with a Strike focused (its preview drawn
/// into the bars), a Defend focused (no preview) and an enemy focused as the target.
#[test]
fn the_focus_moves_nothing_on_the_fight_screen() {
    let strike = fight();
    let mut defend = fight();
    defend.handle_key(KeyCode::Down);
    let hand = &defend.combat().expect("fight").state().player.hand;
    assert_eq!(strike.focused(), Some(Target::Card(hand[0])));
    assert_eq!(defend.focused(), Some(Target::Card(hand[1])));
    let target = targeting();
    for (width, height) in SIZES.into_iter().chain([(60, 20), (40, 14)]) {
        let a = render(&strike, width, height);
        let b = render(&defend, width, height);
        let c = render(&target, width, height);
        assert!(
            previewed(&strike, width, height),
            "{width}x{height}: the Strike's preview\n{a}"
        );
        assert!(
            !previewed(&defend, width, height),
            "{width}x{height}: no preview\n{b}"
        );
        assert!(!anchors(&a).is_empty(), "{width}x{height}:\n{a}");
        assert_eq!(anchors(&a), anchors(&b), "{width}x{height}:\n{a}\n{b}");
        assert_eq!(anchors(&a), anchors(&c), "{width}x{height}:\n{a}\n{c}");
    }
}

/// Playing cards moves nothing: the creatures and End Turn keep their rows while the hand
/// empties (the screen is laid out for a full hand; the rows played cards leave stay blank).
#[test]
fn playing_cards_moves_neither_the_creatures_nor_end_turn() {
    let deck = ["DEFEND_IRONCLAD"; 10];
    for (width, height) in SIZES.into_iter().chain([(60, 20)]) {
        let mut app = fight_with_deck(&deck);
        let end_turn = |screen: &str| screen.lines().position(|l| l.contains("End Turn"));
        let first = render(&app, width, height);
        assert!(end_turn(&first).is_some(), "{width}x{height}:\n{first}");
        for _ in 0..3 {
            press(&mut app, "1");
            let now = render(&app, width, height);
            assert_eq!(
                end_turn(&first),
                end_turn(&now),
                "{width}x{height}:\n{first}\n{now}"
            );
            assert_eq!(
                anchors(&first),
                anchors(&now),
                "{width}x{height}:\n{first}\n{now}"
            );
        }
    }
}

/// The focused card's row opens to its text in full, wrapped, and the enemies and the player
/// stay where they are: the hand's list takes the rows, scrolling if it must.
#[test]
fn the_focused_card_shows_its_whole_text_and_moves_nothing() {
    let deck = ["BASH", "BASH", "BASH", "BASH", "BASH", "BASH"];
    let first = fight_with_deck(&deck);
    let mut second = fight_with_deck(&deck);
    second.handle_key(KeyCode::Down);
    for (width, height) in SIZES.into_iter().chain([(170, 55), (60, 20), (40, 14)]) {
        let a = render(&first, width, height);
        let b = render(&second, width, height);
        assert!(
            a.contains("Vulnerable."),
            "{width}x{height}: the whole text\n{a}"
        );
        // A wide column shows every card's whole text; a narrow one only the focused card's.
        if !ui::scale::of(ratatui::layout::Rect::new(0, 0, width, height)).wide {
            assert_eq!(
                a.matches("Vulnerable.").count(),
                1,
                "{width}x{height}: only the focused one\n{a}"
            );
        }
        assert_eq!(anchors(&a), anchors(&b), "{width}x{height}:\n{a}\n{b}");
        assert!(
            a.contains("▸1 ") && b.contains("▸2 "),
            "{width}x{height}: the focused row is on screen\n{a}\n{b}"
        );
    }
}

/// No size from 20×10 up breaks a screen, with pictures or without.
#[test]
fn small_terminals_draw_every_screen() {
    let apps = [fight(), targeting(), rewards(), map(), shop()];
    for app in &apps {
        for width in [20, 25, 33, 40, 52, 61, 79, 99, 101, 149, 151, 200] {
            for height in [10, 13, 19, 20, 23, 29, 31, 45, 47, 60] {
                render(app, width, height);
            }
        }
    }
}

#[test]
fn reward_map_and_shop_screens_match_their_snapshots() {
    check("rewards", &rewards());
    check("card-reward", &card_reward());
    check("map", &map());
    check("shop", &shop());
}

/// The new stage-4 screens: the shop's open section, the run menu and the deck sheet on the
/// map, and the top bar with a potion belt (one potion asking).
#[test]
fn stage_four_screens_match_their_snapshots() {
    let mut open = shop();
    press(&mut open, "1");
    check("shop-section", &open);
    let mut menu = map();
    menu.handle_key(KeyCode::Tab);
    check("map-menu", &menu);
    let mut deck = map();
    press(&mut deck, "d");
    check("map-deck", &deck);
    check(
        "map-potions",
        &map_app_with_potions(&[Some("FIRE_POTION"), Some("BLOCK_POTION"), None]),
    );
    let mut asking = map_app_with_potions(&[Some("FIRE_POTION"), Some("BLOCK_POTION"), None]);
    press(&mut asking, "c");
    check("map-potion-asking", &asking);
}

/// The shop's open section has a Back button; Esc and the button both return to the sections.
#[test]
fn the_shop_section_has_a_back_button() {
    let mut app = shop();
    press(&mut app, "1");
    let screen = render(&app, 80, 24);
    assert!(
        screen.contains("Esc") && screen.contains("Back"),
        "{screen}"
    );
    let mut tapped = app.clone();
    tapped.perform(UiAction::Pick(9_000));
    assert!(!render(&tapped, 80, 24).contains("Back"));
    app.handle_key(KeyCode::Esc);
    assert!(!render(&app, 80, 24).contains("Back"));
    assert!(matches!(app.run().pending_decision(), RunDecision::Shop(_)));
}

/// Osty has its own HP bar under the Necrobinder's, at every size.
#[test]
fn osty_has_its_own_bar() {
    let run = map_run_as(Character::Necrobinder, 80, 80, &[None, None, None]);
    let mut app = App::with_run_text(SEED, 0, run, text());
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    assert!(app.combat().is_some(), "the first floor is a fight");
    for (width, height) in SIZES {
        let screen = render(&app, width, height);
        let row = screen
            .lines()
            .find(|l| l.contains("Osty"))
            .unwrap_or_else(|| panic!("{width}x{height}: Osty's row\n{screen}"));
        assert!(row.contains('█'), "{width}x{height}: Osty's bar\n{screen}");
    }
}

fn rich(role: sts2_view::text::Role, text: &str) -> sts2_view::text::Rich {
    sts2_view::text::Rich(vec![sts2_view::text::RichSpan {
        fx: None,
        role,
        bold: false,
        text: text.to_owned(),
    }])
}

fn power(name: &str, amount: i32, debuff: bool) -> sts2_view::model::PowerRow {
    sts2_view::model::PowerRow {
        name: name.to_owned(),
        amount,
        debuff,
        shown: true,
        detail: Vec::new(),
        art: String::new(),
        next: 0,
    }
}

/// Five enemies, the third with six powers, the fifth dead.
fn crowded_enemies() -> Vec<sts2_view::model::Block> {
    use sts2_view::model::{Block, EnemyRow, Preview};
    use sts2_view::text::Role;

    let names = [
        "Leaf Slime (M)",
        "Twig Slime (S)",
        "Fuzzy Wurm Crawler",
        "Cultist",
        "Louse",
    ];
    names
        .iter()
        .zip(0_i32..)
        .map(|(name, i)| {
            let alive = i != 4;
            Block::Enemy(Box::new(EnemyRow {
                id: 10 + i.unsigned_abs(),
                art: String::new(),
                name: (*name).to_owned(),
                hp: if alive { 30 - 4 * i } else { 0 },
                max_hp: 33,
                block: if i == 1 { 5 } else { 0 },
                alive,
                dead_state: false,
                escaped: false,
                stolen: None,
                intent: alive.then(|| rich(Role::Attack, "⚔ 6×2")),
                intent_long: alive.then(|| rich(Role::Attack, "⚔ 6×2 damage")),
                powers: if i == 2 {
                    vec![
                        power("Strength", 3, false),
                        power("Ritual", 2, false),
                        power("Vulnerable", 2, true),
                        power("Weak", 1, true),
                        power("Thorns", 4, false),
                        power("Ravenous", 4, false),
                    ]
                } else {
                    Vec::new()
                },
                preview: (alive && i == 0).then_some(Preview::Unchanged),
                target: None,
            }))
        })
        .collect()
}

/// Ten cards, the first selected.
fn crowded_hand() -> Vec<sts2_view::model::Button> {
    use sts2_view::model::{Button, ButtonState, CardFace, Key};
    use sts2_view::text::Role;
    use sts2_view::ui::ids;

    (0..10_u8)
        .map(|i| {
            let (name, kind, effect) = if i % 2 == 0 {
                ("Strike", "Attack", "Deal 6 damage")
            } else {
                ("Defend", "Skill", "Gain 5 Block")
            };
            Button {
                key: Some(Key::Digit((i + 1) % 10)),
                right: Some(rich(Role::Energy, "ϟ1")),
                state: if i == 0 {
                    ButtonState::Selected
                } else {
                    ButtonState::Enabled
                },
                card: Some(CardFace {
                    card: 100 + u32::from(i),
                    cost: "1".to_owned(),
                    name: name.to_owned(),
                    kind: kind.to_owned(),
                    effect: rich(Role::Body, effect),
                    text: rich(Role::Body, effect),
                    ..CardFace::default()
                }),
                ..Button::row(ids::row(usize::from(i)).0, rich(Role::Title, name))
            }
        })
        .collect()
}

/// A fight drawn from a hand-made [`Screen`]: ten cards in the hand, five enemies (the third
/// with six powers, the fifth dead), the player with three powers, an incoming line and the
/// last-turn line. The renderer reads nothing but the screen.
fn crowded_screen() -> Screen {
    use sts2_view::model::{Block, Button, ButtonState, IncomingRow, Key, YouRow};
    use sts2_view::text::Role;
    use sts2_view::ui::ids;

    let mut blocks = crowded_enemies();
    blocks.push(Block::You(YouRow {
        id: 1,
        name: "Ironclad".to_owned(),
        hp: 52,
        max_hp: 80,
        block: 7,
        energy: 3,
        max_energy: 3,
        powers: vec![
            power("Strength", 2, false),
            power("Frail", 1, true),
            power("Metallicize", 3, false),
        ],
        draw: 14,
        discard: 3,
        ..YouRow::default()
    }));
    blocks.push(Block::Incoming(IncomingRow {
        total: 36,
        blocked: 7,
        hp_loss: 29,
        hp_after: 23,
        certain: true,
        ..IncomingRow::default()
    }));
    blocks.push(Block::Last(rich(
        Role::Muted,
        "You played Bash: Leaf Slime −8 HP; the enemies' turn took 4 HP",
    )));
    blocks.push(Block::List {
        frame: true,
        rows: crowded_hand(),
        page: None,
    });
    Screen {
        blocks,
        primary: Some(Button {
            key: Some(Key::Char('e')),
            state: ButtonState::Enabled,
            ..Button::row(ids::END_TURN.0, rich(Role::Key, "End Turn"))
        }),
        ..Screen::default()
    }
}

#[test]
fn a_crowded_fight_matches_its_snapshots() {
    let screen = crowded_screen();
    let app = fight();
    let draw = |width: u16, height: u16| {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
        terminal
            .draw(|f| ui::draw_fight_screen(f, &app, &screen))
            .expect("draw");
        text_of(terminal.backend().buffer(), width)
    };
    for (width, height) in SIZES {
        let drawn = draw(width, height);
        assert!(drawn.contains("End Turn"), "{width}x{height}\n{drawn}");
        assert!(drawn.contains("Leaf Slime"), "{width}x{height}\n{drawn}");
    }
    check_drawn("combat-crowded", draw);
}
