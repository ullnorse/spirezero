//! Plays through the mouse only: every click and drag goes through `Shell::handle_mouse` at the
//! centre of a region the last frame registered, exactly as a terminal would report it, and
//! the frame is drawn again after each event (`ui::render`).

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use sts2_core::combat::{CardId, CreatureId};
use sts2_core::content::events::EventKind;
use sts2_core::run::{
    Act1, MapCoord, MapPointType, Reward, Run, RunAction, RunDecision, RunState, UnlockState,
};
use sts2_core::{Action, Character, Status};
use sts2_tui::app::{App, Input, Overlay, Pending};
use sts2_tui::hit::{Pane, Target, UiAction};
use sts2_tui::shell::{Mode, Shell};
use sts2_tui::text::Localization;
use sts2_tui::ui;

const SEED: &str = "RL2S2";

/// A terminal and the shell drawn in it.
struct Screen {
    terminal: Terminal<TestBackend>,
    shell: Shell,
}

impl Screen {
    fn new(shell: Shell) -> Self {
        let mut screen = Self {
            terminal: Terminal::new(TestBackend::new(150, 46)).expect("backend"),
            shell,
        };
        screen.draw();
        screen
    }

    fn play(app: App) -> Self {
        Self::new(Shell::play(app))
    }

    fn draw(&mut self) {
        ui::render(&mut self.terminal, &mut self.shell).expect("draw");
    }

    fn app(&self) -> &App {
        match self.shell.mode() {
            Mode::Play(app) => app,
            Mode::Select(_) => panic!("not in a run"),
        }
    }

    fn text(&self) -> String {
        let buffer = self.terminal.backend().buffer();
        let width = usize::from(buffer.area.width);
        buffer
            .content()
            .chunks(width)
            .map(|row| row.iter().map(Cell::symbol).collect::<String>() + "\n")
            .collect()
    }

    /// Where `target` was drawn: the middle of its region.
    fn at(&self, target: Target) -> Position {
        let rect = self
            .shell
            .hits()
            .rect_of(target)
            .unwrap_or_else(|| panic!("{target:?} is not on the screen:\n{}", self.text()));
        Position::new(rect.x + rect.width / 2, rect.y + rect.height / 2)
    }

    fn send(&mut self, kind: MouseEventKind, pos: Position) {
        self.shell.handle_mouse(MouseEvent {
            kind,
            column: pos.x,
            row: pos.y,
            modifiers: KeyModifiers::NONE,
        });
        self.draw();
    }

    fn hover(&mut self, target: Target) {
        let pos = self.at(target);
        self.send(MouseEventKind::Moved, pos);
    }

    fn click(&mut self, target: Target) {
        let pos = self.at(target);
        self.send(MouseEventKind::Moved, pos);
        self.send(MouseEventKind::Down(MouseButton::Left), pos);
        self.send(MouseEventKind::Up(MouseButton::Left), pos);
    }

    fn right_click(&mut self) {
        self.send(
            MouseEventKind::Down(MouseButton::Right),
            Position::new(1, 1),
        );
    }

    fn right_click_on(&mut self, target: Target) {
        let pos = self.at(target);
        self.send(MouseEventKind::Down(MouseButton::Right), pos);
    }

    fn drag(&mut self, from: Target, to: Target) {
        let (from, to) = (self.at(from), self.at(to));
        self.send(MouseEventKind::Down(MouseButton::Left), from);
        self.send(MouseEventKind::Drag(MouseButton::Left), to);
        self.send(MouseEventKind::Up(MouseButton::Left), to);
    }

    fn wheel(&mut self, target: Target, down: bool) {
        let pos = self.at(target);
        let kind = if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        };
        self.send(kind, pos);
    }
}

fn press(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.handle_key(KeyCode::Char(c));
    }
}

/// A run that has answered Neow and waits on the map, with plenty of HP, gold and two potions.
fn map_app() -> App {
    App::with_run_text(SEED, 0, map_run(), Rc::new(Localization::empty()))
}

fn map_run() -> Run {
    map_run_of(SEED)
}

/// [`map_run`] on `seed`.
fn map_run_of(seed: &str) -> Run {
    let mut run = Run::new(
        Character::Ironclad,
        seed,
        0,
        UnlockState::all(),
        Act1::Random,
    )
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
    state.hp = 9_999;
    state.max_hp = 9_999;
    state.gold = 5_000;
    state.potions = vec![Some("FIRE_POTION"), Some("BLOCK_POTION"), None];
    run
}

/// The map nodes the run can travel to now.
fn reachable(app: &App) -> Vec<MapCoord> {
    app.run()
        .legal_actions()
        .into_iter()
        .filter_map(|a| match a {
            RunAction::ChooseNode(c) => Some(c),
            _ => None,
        })
        .collect()
}

/// Wins the fight in progress with the keys.
fn win_fight(app: &mut App) {
    for _ in 0..300 {
        if app.combat().is_none() {
            return;
        }
        press(app, "111e");
    }
    panic!("the fight did not end");
}

/// Answers whatever is not the map or a fight with the first choice that works (leaving the
/// rewards), until the map.
fn to_map(app: &mut App) {
    for _ in 0..60 {
        match app.run().pending_decision() {
            RunDecision::Map(_) => return,
            RunDecision::Combat(_) => win_fight(app),
            RunDecision::Rewards(_) => {
                let n = app.menu().iter().filter(|i| i.enabled).count();
                app.perform(UiAction::Pick(n));
                app.perform(UiAction::Yes);
            }
            _ => {
                let n = app.menu().iter().filter(|i| i.enabled).count();
                let next = (1..=n)
                    .map(|i| {
                        let mut copy = app.clone();
                        copy.perform(UiAction::Pick(i));
                        copy
                    })
                    .find(|copy| copy.error().is_none())
                    .expect("a choice that works");
                *app = next;
            }
        }
    }
    panic!("never back on the map");
}

/// A fight on the first floor.
fn fight_app() -> App {
    let mut app = map_app();
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    assert!(app.combat().is_some(), "the first floor is a fight");
    app
}

/// A card of the hand that can be played on the enemy, and one that takes no target.
fn cards(app: &App) -> (Vec<(CardId, CreatureId)>, Vec<CardId>) {
    let mut aimed = Vec::new();
    let mut untargeted = Vec::new();
    for a in app.run().legal_actions() {
        match a {
            RunAction::Combat(Action::PlayCard {
                card,
                target: Some(t),
            }) if t != app.combat().expect("fight").state().player.creature => {
                aimed.push((card, t));
            }
            RunAction::Combat(Action::PlayCard { card, target: None }) => untargeted.push(card),
            _ => {}
        }
    }
    (aimed, untargeted)
}

fn hp(app: &App, id: CreatureId) -> i32 {
    app.combat().expect("fight").state().creature(id).hp
}

fn in_hand(app: &App, card: CardId) -> bool {
    app.combat()
        .expect("fight")
        .state()
        .player
        .hand
        .contains(&card)
}

#[test]
fn a_card_is_played_by_clicking_it_then_its_target() {
    // Seed 12345's first fight has three slimes; `SEED`'s has one enemy, hit at once.
    let mut app = App::with_run_text(
        "12345",
        0,
        map_run_of("12345"),
        Rc::new(Localization::empty()),
    );
    let node = reachable(&app)[0];
    app.perform(UiAction::Travel(node));
    let mut screen = Screen::play(app);
    let (aimed, _) = cards(screen.app());
    let (card, _) = aimed[0];
    let enemy = screen.app().combat().expect("fight").state().enemies[1];
    let before = hp(screen.app(), enemy);
    screen.click(Target::Card(card));
    assert_eq!(
        screen.app().input(),
        &Input::Target(Pending::Card(card)),
        "one click: the card waits for its target"
    );
    assert!(in_hand(screen.app(), card), "not played yet");
    // A right click puts it back; a click on an enemy plays it there.
    screen.right_click();
    assert_eq!(screen.app().input(), &Input::Idle, "right click cancels");
    assert!(in_hand(screen.app(), card));
    screen.click(Target::Card(card));
    screen.click(Target::Creature(enemy));
    assert!(!in_hand(screen.app(), card), "played");
    assert!(hp(screen.app(), enemy) < before, "the enemy was hit");
    assert_eq!(screen.app().input(), &Input::Idle);

    // One enemy: a click on an attack plays it there at once.
    let mut screen = Screen::play(fight_app());
    let (aimed, _) = cards(screen.app());
    let (card, enemy) = aimed[0];
    let before = hp(screen.app(), enemy);
    screen.click(Target::Card(card));
    assert!(!in_hand(screen.app(), card), "played by one click");
    assert!(hp(screen.app(), enemy) < before);
}

#[test]
fn a_card_is_played_by_dragging_it_onto_its_target() {
    let mut screen = Screen::play(fight_app());
    let (aimed, _) = cards(screen.app());
    let (card, enemy) = aimed[0];
    let before = hp(screen.app(), enemy);
    screen.drag(Target::Card(card), Target::Creature(enemy));
    assert!(!in_hand(screen.app(), card), "played");
    assert!(hp(screen.app(), enemy) < before);
}

#[test]
fn a_card_without_a_target_is_played_by_one_click_and_a_drag_can_put_it_back() {
    let mut screen = Screen::play(fight_app());
    let (_, untargeted) = cards(screen.app());
    let card = untargeted[0];
    let block = screen
        .app()
        .combat()
        .expect("fight")
        .state()
        .creature(
            screen
                .app()
                .combat()
                .expect("fight")
                .state()
                .player
                .creature,
        )
        .block;
    screen.click(Target::Card(card));
    assert!(!in_hand(screen.app(), card), "one click plays Defend");
    assert_eq!(screen.app().input(), &Input::Idle);
    let state = screen.app().combat().expect("fight").state();
    assert!(
        state.creature(state.player.creature).block > block,
        "it gained Block"
    );
    // A drag that ends back in the hand puts the card back.
    let (_, untargeted) = cards(screen.app());
    if let Some(&other) = untargeted.first() {
        let pos = screen.at(Target::Card(other));
        screen.send(MouseEventKind::Down(MouseButton::Left), pos);
        screen.send(
            MouseEventKind::Drag(MouseButton::Left),
            Position::new(pos.x, 0),
        );
        screen.send(MouseEventKind::Drag(MouseButton::Left), pos);
        screen.send(MouseEventKind::Up(MouseButton::Left), pos);
        assert!(in_hand(screen.app(), other), "dropped back in the hand");
        assert_eq!(screen.app().input(), &Input::Idle);
    }
}

/// A click on a potion selects it: a Block Potion asks in place on its row and a second click
/// uses it, a click elsewhere puts it back and does nothing else; a Fire Potion with one
/// living enemy is thrown at once (with more it waits for a click on one).
#[test]
fn a_click_selects_a_potion_and_a_click_on_an_enemy_or_the_row_again_uses_it() {
    let mut screen = Screen::play(fight_app());
    let enemies = screen
        .app()
        .combat()
        .expect("fight")
        .state()
        .enemies
        .clone();
    let (_, untargeted) = cards(screen.app());
    let card = untargeted[0];
    // The Block Potion: a click asks, a click elsewhere puts it back without playing anything.
    screen.click(Target::Potion(1));
    assert_eq!(screen.app().input(), &Input::Potion(1));
    assert!(
        screen
            .text()
            .contains("c ◈ Block Potion?  Enter use · t throw away · Esc"),
        "{}",
        screen.text()
    );
    let hand = screen
        .app()
        .combat()
        .expect("fight")
        .state()
        .player
        .hand
        .len();
    screen.click(Target::Card(card));
    assert_eq!(screen.app().input(), &Input::Idle, "put back");
    assert_eq!(
        screen
            .app()
            .combat()
            .expect("fight")
            .state()
            .player
            .hand
            .len(),
        hand,
        "the click that put it back played nothing"
    );
    assert_eq!(screen.app().run().potions()[1], Some("BLOCK_POTION"));
    // A click on its row again uses it.
    screen.click(Target::Potion(1));
    screen.click(Target::Potion(1));
    assert_eq!(screen.app().run().potions()[1], None, "used");
    let state = screen.app().combat().expect("fight").state();
    assert!(state.creature(state.player.creature).block > 0);
    // The Fire Potion: one living enemy, so a click throws it there at once.
    assert_eq!(enemies.len(), 1, "this seed's first fight has one enemy");
    let before = hp(screen.app(), enemies[0]);
    screen.click(Target::Potion(0));
    assert_eq!(screen.app().input(), &Input::Idle);
    assert!(
        hp(screen.app(), enemies[0]) < before,
        "thrown at the one enemy"
    );
    assert_eq!(screen.app().run().potions()[0], None);
    assert!(screen.text().contains("No potions"), "{}", screen.text());
}

#[test]
fn end_turn_and_potions_are_clickable_and_hovering_does_nothing() {
    let mut screen = Screen::play(fight_app());
    let (aimed, _) = cards(screen.app());
    let before = screen.text();
    let (card, _) = aimed.last().copied().expect("an attack");
    let focus = screen.app().focused();
    screen.hover(Target::Card(card));
    assert_eq!(screen.shell.hover(), Some(Target::Card(card)));
    assert_eq!(
        screen.app().focused(),
        focus,
        "the pointer moving never moves the focus (a click does)"
    );
    assert_eq!(
        before.lines().count(),
        screen.text().lines().count(),
        "nothing popped up"
    );
    assert!(!screen.text().contains("Attack, cost"), "no tooltip");
    let enemy = screen.app().combat().expect("fight").state().enemies[0];

    // Fire Potion: a click uses it, then a target.
    let before = hp(screen.app(), enemy);
    screen.click(Target::Potion(0));
    if matches!(screen.app().input(), Input::Target(Pending::Potion(0))) {
        screen.click(Target::Creature(enemy));
    }
    assert!(hp(screen.app(), enemy) < before, "the potion hit");
    assert_eq!(screen.app().run().potions()[0], None);
    // Block Potion: thrown away from the menu's row, which asks in place first.
    screen.click(Target::Button(UiAction::Toggle(Overlay::Menu)));
    screen.click(Target::Button(UiAction::TossPotion(1)));
    assert!(
        screen
            .text()
            .contains("Throw away Block Potion? Enter again"),
        "{}",
        screen.text()
    );
    assert_eq!(screen.app().run().potions()[1], Some("BLOCK_POTION"));
    screen.click(Target::Button(UiAction::TossPotion(1)));
    assert_eq!(screen.app().run().potions()[1], None);
    assert_eq!(screen.app().overlay(), None, "the menu closed on it");

    let round = screen.app().combat().expect("fight").state().round;
    screen.click(Target::Button(UiAction::EndTurn));
    let after = screen.app().combat().map(|c| c.state().round);
    assert!(after.is_none_or(|r| r > round), "the turn ended");
}

#[test]
fn the_map_is_travelled_by_clicking_a_node() {
    let mut screen = Screen::play(map_app());
    let node = reachable(screen.app())[0];
    screen.hover(Target::MapNode(node));
    assert!(
        !screen.text().contains("Click to travel here."),
        "no tooltip"
    );
    screen.wheel(Target::Pane(Pane::Map), false);
    screen.click(Target::MapNode(node));
    assert_eq!(screen.app().run().state().coord(), Some(node));
}

#[test]
fn rewards_are_picked_and_left_by_clicking() {
    let mut app = fight_app();
    win_fight(&mut app);
    let mut screen = Screen::play(app);
    let RunDecision::Rewards(rewards) = screen.app().run().pending_decision() else {
        panic!("the fight is won");
    };
    let gold = matches!(rewards.first(), Some(Reward::Gold(_)));
    let count = rewards.len();
    let before = screen.app().run().state().gold;
    screen.hover(Target::Button(UiAction::Pick(1)));
    assert_eq!(
        screen.shell.hover(),
        Some(Target::Button(UiAction::Pick(1))),
        "the entry is lit"
    );
    screen.click(Target::Button(UiAction::Pick(1)));
    let RunDecision::Rewards(rewards) = screen.app().run().pending_decision() else {
        panic!("more rewards are left");
    };
    assert_eq!(rewards.len(), count - 1, "the first reward was taken");
    if gold {
        assert!(screen.app().run().state().gold > before);
    }
    // Leave asks first, in its own label: a click elsewhere puts it back, a second click on
    // it leaves.
    let n = screen.app().menu().iter().filter(|i| i.enabled).count();
    screen.click(Target::Button(UiAction::Pick(n)));
    assert!(screen.app().confirm_prompt().is_some());
    assert!(
        screen.text().contains("Leave without taking"),
        "asked in place"
    );
    screen.right_click();
    assert!(screen.app().confirm_prompt().is_none(), "put back");
    assert!(screen.app().overlay().is_none(), "the click only answered");
    screen.click(Target::Button(UiAction::Pick(n)));
    screen.click(Target::Button(UiAction::Pick(n)));
    assert!(matches!(
        screen.app().run().pending_decision(),
        RunDecision::Map(_)
    ));
}

/// The path from where the run stands to the nearest shop.
fn path_to_shop(state: &RunState) -> Vec<MapCoord> {
    let start = state.coord().unwrap_or_else(|| state.map.start().coord);
    let mut came: HashMap<MapCoord, MapCoord> = HashMap::new();
    let mut queue = VecDeque::from([start]);
    while let Some(at) = queue.pop_front() {
        if state
            .map
            .point(at)
            .is_some_and(|p| p.kind == MapPointType::Shop)
        {
            let mut path = vec![at];
            while let Some(&from) = came.get(path.last().expect("path")) {
                path.push(from);
            }
            path.pop();
            path.reverse();
            return path;
        }
        for child in &state.map.point(at).expect("node").children {
            if let std::collections::hash_map::Entry::Vacant(e) = came.entry(child) {
                e.insert(at);
                queue.push_back(child);
            }
        }
    }
    panic!("no shop in the act");
}

#[test]
fn the_shop_sells_by_clicking() {
    let mut app = map_app();
    for node in path_to_shop(app.run().state()) {
        to_map(&mut app);
        app.perform(UiAction::Travel(node));
    }
    let RunDecision::Shop(shop) = app.run().pending_decision() else {
        panic!("in the shop");
    };
    let cards = shop.cards.iter().filter(|c| c.card.is_some()).count();
    let mut screen = Screen::play(app);
    let gold = screen.app().run().state().gold;
    let deck = screen.app().run().state().deck.len();
    // The first level lists the sections; the first opens the cards.
    screen.click(Target::Button(UiAction::Pick(1)));
    screen.click(Target::Button(UiAction::Pick(1)));
    assert_eq!(screen.app().run().state().gold, gold, "a click asks first");
    assert!(screen.text().contains("Buy for"), "asked in the row");
    screen.click(Target::Button(UiAction::Pick(1)));
    assert!(screen.app().run().state().gold < gold, "paid");
    assert_eq!(screen.app().run().state().deck.len(), deck + 1, "bought");
    let RunDecision::Shop(shop) = screen.app().run().pending_decision() else {
        panic!("still in the shop");
    };
    assert_eq!(
        shop.cards.iter().filter(|c| c.card.is_some()).count(),
        cards - 1
    );
}

#[test]
fn the_log_stops_scrolling_at_its_first_line() {
    let mut screen = Screen::play(fight_app());
    // Enough turns to fill the log's popup.
    for _ in 0..12 {
        if screen.app().combat().is_some() {
            screen.click(Target::Button(UiAction::EndTurn));
        }
    }
    // The log is behind `l`.
    screen.shell.handle_key(KeyCode::Char('l'));
    screen.draw();
    for _ in 0..200 {
        screen.wheel(Target::Pane(Pane::Log), false);
    }
    let (_, max) = screen
        .shell
        .hits()
        .scroll_limit(Pane::Log)
        .expect("the log was drawn");
    assert!(max > 0, "the fight has more log than fits");
    assert_eq!(screen.app().log_scroll(), usize::try_from(max).unwrap_or(0));
    // One notch (three lines) back follows at once, with no dead presses.
    screen.wheel(Target::Pane(Pane::Log), true);
    assert_eq!(
        screen.app().log_scroll(),
        usize::try_from(max).unwrap_or(0) - 3
    );
}

#[test]
fn overlays_open_scroll_and_close_by_mouse() {
    let mut screen = Screen::play(fight_app());
    // The deck is in the menu behind the top bar's ≡.
    screen.click(Target::Button(UiAction::Toggle(Overlay::Menu)));
    assert_eq!(screen.app().overlay(), Some(Overlay::Menu));
    screen.click(Target::Button(UiAction::Toggle(Overlay::Deck)));
    assert_eq!(screen.app().overlay(), Some(Overlay::Deck));
    screen.wheel(Target::Pane(Pane::Overlay), true);
    // Nothing under the list can be clicked: a click beside it closes it.
    screen.send(
        MouseEventKind::Down(MouseButton::Left),
        Position::new(0, 20),
    );
    screen.send(MouseEventKind::Up(MouseButton::Left), Position::new(0, 20));
    assert_eq!(screen.app().overlay(), None);
    screen.click(Target::Button(UiAction::Toggle(Overlay::DrawPile)));
    assert_eq!(screen.app().overlay(), Some(Overlay::DrawPile));
    screen.click(Target::Button(UiAction::Back));
    assert_eq!(screen.app().overlay(), None);
}

#[test]
fn a_right_click_on_a_card_or_an_enemy_opens_its_detail() {
    let mut screen = Screen::play(fight_app());
    let (aimed, _) = cards(screen.app());
    let (card, enemy) = aimed[0];
    screen.right_click_on(Target::Card(card));
    assert!(
        matches!(screen.app().overlay(), Some(Overlay::Inspect(_))),
        "the card's detail"
    );
    assert!(
        screen.text().contains("energy"),
        "its cost line is in the panel beside the column:\n{}",
        screen.text()
    );
    screen.right_click();
    assert_eq!(screen.app().overlay(), None, "a right click closes it");
    screen.right_click_on(Target::Creature(enemy));
    assert!(matches!(screen.app().overlay(), Some(Overlay::Inspect(_))));
    assert!(screen.text().contains("HP "), "{}", screen.text());
}

#[test]
fn the_character_select_works_by_mouse() {
    let mut screen = Screen::new(Shell::select(Rc::new(Localization::empty())));
    screen.click(Target::Button(UiAction::SelectCharacter(2)));
    screen.wheel(Target::Pane(Pane::Ascension), false);
    screen.click(Target::Button(UiAction::Ascension(true)));
    screen.click(Target::Button(UiAction::Ascension(false)));
    screen.click(Target::Button(UiAction::Start));
    let app = screen.app();
    assert_eq!(app.run().state().character, Character::Regent);
    assert_eq!(app.run().state().ascension, 1);
}

#[test]
fn a_fight_card_choice_is_answered_by_clicking_a_card() {
    let mut run = map_run();
    run.loadout_relic("TOASTY_MITTENS").expect("ported");
    let node = run
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, RunAction::ChooseNode(_)))
        .expect("a node");
    run.apply(node).expect("legal");
    assert!(
        run.combat().is_some_and(|c| c.status() == Status::Decision),
        "Toasty Mittens asks at the first turn"
    );
    let mut screen = Screen::play(App::with_run_text(
        SEED,
        0,
        run,
        Rc::new(Localization::empty()),
    ));
    let Some(Action::Resolve(sts2_core::combat::Decision::Pick(card))) =
        screen.app().decision_options().first().cloned()
    else {
        panic!("a card to pick");
    };
    screen.click(Target::Choice(1, card));
    let combat = screen.app().combat().expect("fight");
    assert_ne!(combat.status(), Status::Decision, "answered");
    assert!(combat.state().player.exhaust.contains(&card), "exhausted");
}

#[test]
fn a_deck_selection_is_answered_by_clicking_a_card() {
    let mut run = map_run();
    run.debug_enter_event(EventKind::Wellspring)
        .expect("Wellspring is ported");
    let bathe = run.legal_actions().into_iter().nth(1).expect("Bathe");
    run.apply(bathe).expect("legal");
    assert!(matches!(run.pending_decision(), RunDecision::CardSelect(_)));
    let mut screen = Screen::play(App::with_run_text(
        SEED,
        0,
        run,
        Rc::new(Localization::empty()),
    ));
    screen.click(Target::Button(UiAction::Pick(1)));
    assert_eq!(screen.app().error(), None);
    assert!(
        matches!(screen.app().run().pending_decision(), RunDecision::Map(_)),
        "the selection is answered"
    );
}
