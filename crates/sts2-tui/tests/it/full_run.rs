//! Drives [`App`] with scripted key presses through whole Act 1 runs into Act 2, making the greedy policy's
//! choices (`sts2-diff`), and checks after every step that the run is exactly the one the policy
//! played on its own session. Every screen is rendered along the way.

use std::collections::BTreeSet;
use std::rc::Rc;
use sts2_core::Character;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::KeyCode;
use sts2_core::combat::Decision;
use sts2_core::run::{Reward, Run, RunAction, RunDecision, UnlockState};
use sts2_core::{Action, Status};
use sts2_diff::greedy::Greedy;
use sts2_diff::run_session::RunSession;
use sts2_tui::app::{App, Input, Overlay};
use sts2_tui::hit::Target;
use sts2_tui::keys;
use sts2_tui::shell::Shell;
use sts2_tui::text::Localization;
use sts2_tui::ui;

/// Seeds whose greedy run beats Act 1's boss as the Necrobinder (`POLICY=greedy tools/diff_run.sh`):
/// `RL2S<n>` with policy seed `n`.
const NECROBINDER_SEEDS: [&str; 3] = ["RL2S2", "RL2S11", "RL2S19"];
/// A seed whose greedy run beats Act 1's boss for every character (found by scanning `RL2S0..40` with
/// Overgrowth forced, as the policy's session needs).
const ALL_CHARACTERS_SEED: &str = "RL2S19";
const MAX_STEPS: usize = 3000;

fn render(app: &App) -> String {
    render_at(app, 110, 42)
}

fn render_at(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
    terminal.draw(|f| ui::draw(f, app)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(Cell::symbol)
        .collect()
}

/// Presses the key that picks the `n`th (1-based) entry: a digit (`0` the tenth); past ten
/// a menu is paged (`PgDn` to the page, then the digit) and a card choice of the fight is
/// reached by the focus and Enter.
fn press_number(app: &mut App, n: usize) {
    let digit =
        |k: usize| char::from_digit(u32::try_from(k % 10).expect("a digit"), 10).expect("a digit");
    if app.combat().is_some() {
        if n <= 10 {
            app.handle_key(KeyCode::Char(digit(n)));
            return;
        }
        let options = app.decision_options();
        if let Some(Action::Resolve(Decision::Pick(card))) = options.get(n - 1) {
            app.focus_on(Target::Choice(n, *card));
            app.handle_key(KeyCode::Enter);
        }
        return;
    }
    // A menu: the digit picks on the page the focus is on, so turn to the entry's page first
    // (the main button is `0` on every page).
    let paging = app.paging();
    if let Some(want) = paging.page_of(n) {
        let have = app.page();
        let turn = if want > have {
            KeyCode::PageDown
        } else {
            KeyCode::PageUp
        };
        for _ in 0..want.abs_diff(have) {
            app.handle_key(turn);
        }
    }
    let key = paging.key(n).chars().next().expect("a key");
    app.handle_key(KeyCode::Char(key));
}

/// The card reward `action` answers, if it answers one: its screen must be open first.
fn card_reward_of(app: &App, action: &RunAction) -> Option<usize> {
    let RunDecision::Rewards(rewards) = app.run().pending_decision() else {
        return None;
    };
    let i = match action {
        RunAction::PickCard { reward, .. } | RunAction::CardAlternative { reward, .. } => *reward,
        RunAction::SkipReward(i) => *i,
        _ => return None,
    };
    matches!(rewards.get(i), Some(Reward::Card(_))).then_some(i)
}

/// Opens the card reward's screen when `action` answers one and it is not open yet.
fn open_card_reward(app: &mut App, action: &RunAction) {
    if let Some(i) = card_reward_of(app, action)
        && app.open_reward() != Some(i)
    {
        let row = app
            .menu()
            .iter()
            .filter(|item| item.enabled)
            .position(|item| item.open == Some(i))
            .expect("the card reward's row");
        press_number(app, row + 1);
    }
}

/// Puts the shop on the level that shows `action`: its section open, or the first level.
fn open_shop_section(app: &mut App, action: &RunAction) {
    if !matches!(app.run().pending_decision(), RunDecision::Shop(_)) {
        return;
    }
    let want = sts2_view::menu::buy_section(app.run(), action);
    if app.open_section() == want {
        return;
    }
    if app.open_section().is_some() {
        app.handle_key(KeyCode::Esc);
    }
    if let Some(section) = want {
        let row = app
            .menu()
            .iter()
            .filter(|item| item.enabled)
            .position(|item| item.section.map(|(s, _)| s) == Some(section))
            .expect("the section's row");
        press_number(app, row + 1);
    }
}

/// Picks `action` from the combat's pending card choice by its number.
fn press_option(app: &mut App, action: &Action) {
    let index = app
        .decision_options()
        .iter()
        .position(|a| a == action)
        .expect("an offered answer");
    press_number(app, index + 1);
}

/// The keys that make `action`, pressed.
fn press_action(app: &mut App, action: &RunAction) {
    match action {
        RunAction::Combat(Action::PlayCard { card, target }) => {
            let state = app.combat().expect("a fight").state();
            let index = state
                .player
                .hand
                .iter()
                .position(|c| c == card)
                .expect("in hand");
            press_number(app, index + 1);
            if let (Some(target), Input::Target(_)) = (target, app.input()) {
                let state = app.combat().expect("a fight").state();
                let slot = state
                    .enemies
                    .iter()
                    .position(|e| e == target)
                    .expect("an enemy");
                press_number(app, slot + 1);
            }
        }
        RunAction::Combat(Action::EndTurn) => app.handle_key(KeyCode::Char('e')),
        // The policy answers a card choice whole; the TUI picks the cards one at a time.
        RunAction::Combat(Action::Resolve(Decision::Cards(cards))) => {
            for card in cards {
                let pick = Action::Resolve(Decision::Pick(*card));
                press_option(app, &pick);
            }
            if app.combat().is_some_and(|c| c.pending_decision().is_some()) {
                press_option(app, &Action::Resolve(Decision::Done));
            }
        }
        RunAction::Combat(action @ Action::Resolve(_)) => press_option(app, action),
        RunAction::Combat(Action::UsePotion { slot, target }) => {
            app.handle_key(KeyCode::Char(keys::potion_key(*slot)));
            if let (Some(target), Input::Target(_)) = (target, app.input()) {
                let state = app.combat().expect("a fight").state();
                let at = state
                    .enemies
                    .iter()
                    .position(|e| e == target)
                    .expect("an enemy");
                press_number(app, at + 1);
            }
            // A potion that needs no target asks in place: Enter uses it.
            if matches!(app.input(), Input::Potion(_)) {
                app.handle_key(KeyCode::Enter);
            }
        }
        RunAction::UsePotion(slot) => {
            app.handle_key(KeyCode::Char(keys::potion_key(*slot)));
            if matches!(app.input(), Input::Potion(_)) {
                app.handle_key(KeyCode::Enter);
            }
        }
        // Cancelling a selection is Esc (a free card's Skip is a row).
        RunAction::CancelSelect
            if !app
                .menu()
                .iter()
                .any(|i| i.enabled && i.action == RunAction::CancelSelect) =>
        {
            app.handle_key(KeyCode::Esc);
        }
        _ => {
            // A card reward's screen hides the other rewards: Esc is the way back to them.
            if app.open_reward().is_some() && card_reward_of(app, action).is_none() {
                app.handle_key(KeyCode::Esc);
            }
            open_card_reward(app, action);
            open_shop_section(app, action);
            let index = app
                .menu()
                .iter()
                .filter(|i| i.enabled)
                .position(|i| i.action == *action)
                .unwrap_or_else(|| panic!("{action:?} is not on the menu"));
            press_number(app, index + 1);
        }
    }
    // A choice that throws something away asks first: yes.
    if app.confirm_prompt().is_some() {
        app.handle_key(KeyCode::Enter);
    }
    assert_eq!(app.error(), None, "{action:?} was rejected");
}

/// What sts2-cli does on its own after an action, as the policy's session scripts it
/// (`run_session::advance`): the first card reward answered closes the screen, gold, relics and
/// potions that fit are claimed, a treasure room's relic is taken.
fn settle(app: &mut App, answered_card_reward: bool, seen: &mut BTreeSet<&'static str>) {
    if answered_card_reward && matches!(app.run().pending_decision(), RunDecision::Rewards(_)) {
        press_action(app, &RunAction::Proceed);
    }
    loop {
        smoke(app, seen);
        let next = match app.run().pending_decision() {
            RunDecision::Rewards(rewards) => {
                let free_slot = app.run().state().potions.contains(&None);
                let claimable = rewards.iter().position(|r| match r {
                    Reward::Gold(_)
                    | Reward::Relic(_)
                    | Reward::WaxRelic(_)
                    | Reward::SpecialCard(_) => true,
                    Reward::Potion(_) => free_slot,
                    Reward::Card(_) => false,
                });
                match claimable {
                    Some(i) => RunAction::ClaimReward(i),
                    None if rewards.iter().any(|r| matches!(r, Reward::Card(_))) => break,
                    None => RunAction::Proceed,
                }
            }
            RunDecision::Treasure(Some(_)) => RunAction::TakeRelic,
            RunDecision::Treasure(None) => RunAction::Proceed,
            _ => break,
        };
        press_action(app, &next);
    }
}

/// The policy's next action, as `sts2-diff run-gen --policy greedy` picks it; `None` where it
/// stops.
fn next_action(session: &RunSession, planner: &mut Greedy) -> Option<RunAction> {
    match session.run().pending_decision() {
        RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_) => return None,
        _ => {}
    }
    if let Some(answer) = session.run().combat().and_then(Greedy::answer) {
        let action = RunAction::Combat(answer);
        return session.check(&action).ok().map(|()| action);
    }
    let mut modeled: Vec<RunAction> = session
        .run()
        .legal_actions()
        .into_iter()
        .filter(|a| session.check(a).is_ok())
        .collect();
    if matches!(session.run().pending_decision(), RunDecision::Ancient(_)) {
        let onward: Vec<RunAction> = modeled
            .iter()
            .filter(|a| session.leads_somewhere(a))
            .cloned()
            .collect();
        if !onward.is_empty() {
            modeled = onward;
        }
    }
    (!modeled.is_empty()).then(|| planner.choose(session.run(), &modeled))
}

/// The screen kind and a text its render must contain.
fn screen(run: &Run) -> (&'static str, &'static str) {
    match run.pending_decision() {
        RunDecision::Ancient(_) => ("neow", "blessing"),
        RunDecision::Event(_) => ("event", "Choose an option"),
        RunDecision::Map(_) => ("map", "1  "),
        RunDecision::Combat(_) => ("combat", "█"),
        RunDecision::Rewards(_) => ("rewards", "Rewards"),
        RunDecision::CardSelect(_) => ("card select", "pick"),
        RunDecision::Bundles(_) => ("bundles", "Bundles"),
        RunDecision::RestSite(_) => ("rest site", "Rest site"),
        RunDecision::Treasure(_) => ("treasure", "Treasure"),
        RunDecision::Shop(_) => ("shop", "Shop"),
        RunDecision::Unsupported(_) => ("unsupported", "not ported"),
        RunDecision::Stopped(_) => ("stopped", "STOPPED"),
        RunDecision::GameOver => ("game over", "DEFEAT"),
        RunDecision::Victory => ("victory", "VICTORY"),
    }
}

/// Renders the app's screen and checks the header and the screen's own text are there.
fn smoke(app: &App, seen: &mut BTreeSet<&'static str>) {
    let (kind, needle) = screen(app.run());
    let text = render(app);
    // The top bar is on every screen: HP, gold, the floor, the menu.
    for header in ["♥", "◉", "Floor", "≡"] {
        assert!(text.contains(header), "{kind}: HUD lacks {header}");
    }
    // A card reward open as its own screen has its own title.
    let card_screen = kind == "rewards" && text.contains("Choose a card");
    // A shop section open is the shop's second level, titled by the section.
    let section_screen = kind == "shop"
        && ["Cards", "Colorless cards", "Relics", "Potions"]
            .iter()
            .any(|s| text.contains(s));
    assert!(
        text.contains(needle) || card_screen || section_screen,
        "{kind}: screen lacks {needle}"
    );
    // No screen may panic in a tiny terminal.
    for (width, height) in [(1, 1), (7, 3), (20, 8), (40, 12), (60, 20)] {
        render_at(app, width, height);
    }
    let in_turn = app
        .combat()
        .is_some_and(|c| c.status() == Status::PlayerTurn);
    if kind == "combat" && in_turn {
        combat_hud(app, &text);
    }
    seen.insert(kind);
}

/// What a fight's screen shows for the character: energy and the piles (unless a prompt takes
/// their line), the End Turn button, and its own resources.
fn combat_hud(app: &App, text: &str) {
    let quiet = *app.input() == Input::Idle && app.error().is_none() && app.number().is_empty();
    for part in ["ϟ", "Draw ", "Discard "] {
        assert!(!quiet || text.contains(part), "combat lacks {part}");
    }
    assert!(text.contains("End Turn"), "combat lacks End Turn");
    let character = app.run().state().character;
    let resource = match character {
        Character::Defect => Some("Orbs"),
        Character::Regent => Some("★"),
        Character::Necrobinder => Some("Osty"),
        Character::Ironclad | Character::Silent => None,
    };
    if let Some(resource) = resource {
        assert!(
            text.contains(resource),
            "{character:?}'s fight lacks {resource}"
        );
    }
}

/// A fresh app with no game text, so screens show ids and engine names whatever is on disk.
fn plain_app(seed: &str) -> App {
    plain_app_of(Character::Necrobinder, seed)
}

fn plain_app_of(character: Character, seed: &str) -> App {
    App::with_text(character, seed, 0, Rc::new(Localization::empty())).expect("run")
}

/// Plays `seed` with the greedy policy through the TUI until it ends or `stop` holds; returns the
/// app and the screens seen.
fn play_until(
    character: Character,
    seed: &str,
    stop: &dyn Fn(&App) -> bool,
) -> (App, BTreeSet<&'static str>) {
    let run = Run::new(
        character,
        seed,
        0,
        UnlockState::all(),
        sts2_core::run::Act1::Overgrowth,
    )
    .expect("run")
    .with_events()
    .in_test_mode();
    let mut app = App::with_run(seed, 0, run);
    let mut session = RunSession::new(character, seed, 0, false, sts2_core::run::Act1::Overgrowth)
        .expect("session");
    let mut planner = Greedy::default();
    let mut drawn = BTreeSet::new();
    assert_eq!(app.run(), session.run());
    for step in 0..MAX_STEPS {
        smoke(&app, &mut drawn);
        if stop(&app) {
            return (app, drawn);
        }
        let Some(action) = next_action(&session, &mut planner) else {
            return (app, drawn);
        };
        let answered_card_reward = match (&action, session.run().pending_decision()) {
            (RunAction::PickCard { .. }, _) => true,
            (RunAction::SkipReward(i), RunDecision::Rewards(rewards)) => {
                matches!(rewards.get(*i), Some(Reward::Card(_)))
            }
            _ => false,
        };
        press_action(&mut app, &action);
        settle(&mut app, answered_card_reward, &mut drawn);
        session.apply(&action).expect("the session accepts it");
        assert!(
            app.run() == session.run(),
            "seed {seed}: the app's run differs from the policy's after step {step}: {action:?}"
        );
    }
    panic!("seed {seed}: no end within {MAX_STEPS} steps");
}

/// Plays `seeds` for `character` through the keys; every run must beat Act 1's boss and enter
/// Act 2's map screen, whose only next point is Hive's ancient.
fn act_complete(character: Character, seeds: &[&str]) {
    let mut seen = BTreeSet::new();
    for seed in seeds {
        let (app, screens) = play_until(character, seed, &|app| app.run().state().act_index == 1);
        let s = app.run().state();
        assert_eq!(
            app.run().pending_decision(),
            RunDecision::Map(std::iter::once(s.map.start().coord).collect()),
            "{character:?} on seed {seed} should enter Act 2"
        );
        let mut map = app.clone();
        map.handle_key(KeyCode::Char('m'));
        assert!(
            render(&map).contains("Act 2 map"),
            "the full map shows Act 2"
        );
        seen.extend(screens);
    }
    for kind in ["neow", "map", "combat", "rewards", "rest site"] {
        assert!(
            seen.contains(kind),
            "{character:?}: no {kind} screen was drawn: {seen:?}"
        );
    }
}

#[test]
fn necrobinder_greedy_runs_reach_act_complete_through_the_keys() {
    act_complete(Character::Necrobinder, &NECROBINDER_SEEDS);
}

#[test]
fn ironclad_greedy_run_reaches_act_complete_through_the_keys() {
    act_complete(Character::Ironclad, &[ALL_CHARACTERS_SEED]);
}

#[test]
fn silent_greedy_run_reaches_act_complete_through_the_keys() {
    act_complete(Character::Silent, &[ALL_CHARACTERS_SEED]);
}

#[test]
fn defect_greedy_run_reaches_act_complete_through_the_keys() {
    act_complete(Character::Defect, &[ALL_CHARACTERS_SEED]);
}

#[test]
fn regent_greedy_run_reaches_act_complete_through_the_keys() {
    act_complete(Character::Regent, &[ALL_CHARACTERS_SEED]);
}

#[test]
fn dying_ends_on_the_game_over_screen() {
    let mut app = plain_app("RL2S2");
    let mut seen = BTreeSet::new();
    // Always take the first choice and end every turn: the first fight kills the player.
    for _ in 0..2000 {
        smoke(&app, &mut seen);
        if app.is_over() {
            break;
        }
        let before = app.run().clone();
        let in_turn = app
            .combat()
            .is_some_and(|c| c.status() == Status::PlayerTurn);
        let key = if in_turn { "e" } else { "1" };
        for c in key.chars() {
            app.handle_key(KeyCode::Char(c));
        }
        if app.run() == &before {
            // The first choice is not ported or not legal: try the next ones.
            for n in 2..=9 {
                app.handle_key(KeyCode::Char(char::from_digit(n, 10).expect("digit")));
                if app.run() != &before {
                    break;
                }
            }
        }
    }
    assert!(
        app.is_over() && seen.contains("game over"),
        "the fight should be lost: {seen:?}"
    );
}

/// Walks seeds taking random legal choices (through the keys) until every screen kind that the
/// greedy runs do not reach has been drawn. The stopped-at-unported-content banner has no seed
/// found yet; it shares its drawing with the game-over banner. The unsupported-content screen is no
/// longer reachable now that all Act 1 content is ported, so it is not required here.
#[test]
fn random_walks_draw_every_screen_kind() {
    let mut seen = BTreeSet::new();
    let wanted = [
        "bundles",
        "game over",
        "treasure",
        "event",
        "shop",
        "card select",
    ];
    for n in 0..80_u64 {
        let name = format!("RL2S{n}");
        let mut app = plain_app(&name);
        let mut rng = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        for _ in 0..800 {
            smoke(&app, &mut seen);
            if app.is_over() {
                break;
            }
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let count = match app.combat() {
                Some(c) if c.status() == Status::PlayerTurn => c.state().player.hand.len() + 1,
                Some(c) => app
                    .decision_options()
                    .len()
                    .max(c.state().player.hand.len()),
                None => app.menu().iter().filter(|i| i.enabled).count(),
            };
            if count == 0 {
                break;
            }
            let pick = usize::try_from(rng % count as u64).expect("small") + 1;
            if app
                .combat()
                .is_some_and(|c| c.status() == Status::PlayerTurn)
                && pick == count
            {
                app.handle_key(KeyCode::Char('e'));
            } else {
                press_number(&mut app, pick);
                // A card that needs a target: take the first enemy.
                if matches!(app.input(), Input::Target(_)) {
                    press_number(&mut app, 1);
                }
            }
        }
        if wanted.iter().all(|k| seen.contains(k)) {
            break;
        }
    }
    for kind in wanted {
        assert!(seen.contains(kind), "no {kind} screen was drawn: {seen:?}");
    }
}

#[test]
fn a_held_potion_is_thrown_away_from_the_menu() {
    let holding = |app: &App| app.run().potions().iter().any(Option::is_some);
    let (mut app, _) = play_until(Character::Necrobinder, "RL2S2", &holding);
    assert!(holding(&app), "the greedy run picks up a potion");
    let slot = app
        .run()
        .potions()
        .iter()
        .position(Option::is_some)
        .expect("a potion");
    let before = app.run().potions().iter().flatten().count();
    app.handle_key(KeyCode::Tab);
    assert!(
        render(&app).contains(&format!(" {}  Throw away", slot + 1)),
        "the row prints its key (the belt slot)\n{}",
        render(&app)
    );
    // The row asks in place; its number again throws the potion away.
    press_number(&mut app, slot + 1);
    assert!(
        render(&app).contains(&format!("? press {}", slot + 1)),
        "{}",
        render(&app)
    );
    assert_eq!(
        app.overlay(),
        Some(Overlay::Menu),
        "the menu stays while it asks"
    );
    press_number(&mut app, slot + 1);
    assert_eq!(app.error(), None);
    assert_eq!(app.overlay(), None, "the menu closed");
    assert_eq!(app.run().potions().iter().flatten().count(), before - 1);
    assert_eq!(app.run().potions()[slot], None);
}

#[test]
fn game_text_shows_on_every_screen_kind_and_falls_back_without_it() {
    // A synthetic table: only the ids of the first screens, in made-up words.
    let loc = Localization::from_json(&[
        (
            "cards",
            r#"{"STRIKE_NECROBINDER.description": "Hit [gold]{Damage:diff()}[/gold] zorps.",
                "DEFEND_NECROBINDER.description": "Shield {Block:diff()} zorps."}"#,
        ),
        (
            "relics",
            r#"{"BOUND_PHYLACTERY.description": "A made-up relic blurb."}"#,
        ),
    ]);
    let mut app = App::with_text(Character::Necrobinder, "RL2S2", 0, Rc::new(loc)).expect("run");
    let mut saw_card_text = false;
    for _ in 0..40 {
        let text = render(&app);
        if app.combat().is_some() {
            saw_card_text = text.contains("zorps");
            break;
        }
        let first = app
            .menu()
            .iter()
            .position(|i| i.enabled)
            .expect("an option");
        press_number(&mut app, first + 1);
    }
    assert!(saw_card_text, "the hand shows card text with its numbers");
    // Wrapped on the card: `Hit 6` / `zorps.`.
    assert!(render(&app).contains("Hit 6"));
    app.handle_key(KeyCode::Char('v'));
    assert!(render(&app).contains("made-up relic blurb"));
    app.handle_key(KeyCode::Char('i'));
    assert!(render(&app).contains("Potions"));
    app.handle_key(KeyCode::Esc);
    app.handle_key(KeyCode::Char('d'));
    assert!(render(&app).contains("Shield 5 zorps."));

    let bare = plain_app("RL2S2");
    assert!(!render(&bare).contains("blurb"));
}

/// The first fight of `character` on `seed`, reached by taking the first working choice on
/// each screen.
fn first_fight(character: Character, seed: &str) -> App {
    let mut app = plain_app_of(character, seed);
    for _ in 0..40 {
        if app.combat().is_some() {
            return app;
        }
        let options = app.menu().iter().filter(|i| i.enabled).count();
        let next = (1..=options).find_map(|n| {
            let mut copy = app.clone();
            press_number(&mut copy, n);
            copy.error().is_none().then_some(copy)
        });
        app = next.expect("a choice that works");
    }
    panic!("{character:?}: no fight within 40 choices");
}

#[test]
fn every_character_has_its_combat_hud() {
    for character in Character::ALL {
        let app = first_fight(character, "RL2S2");
        let text = render(&app);
        combat_hud(&app, &text);
        // The hand is listed, a row per card.
        let state = app.combat().expect("a fight").state();
        let first = sts2_tui::names::card(state, state.player.hand[0]);
        assert!(text.contains(&first), "{character:?}");
    }
}

#[test]
fn the_defect_shows_orbs_with_passive_and_evoke_values() {
    let app = first_fight(Character::Defect, "RL2S2");
    // Cracked Core channels a Lightning orb at the start of the fight: passive 3, evoke 8.
    assert!(render(&app).contains("Lightning 3/8"));
}

#[test]
fn the_regent_shows_stars_and_star_costs() {
    let app = first_fight(Character::Regent, "RL2S2");
    let text = render(&app);
    assert!(text.contains("★ 3"), "the Regent starts a fight with stars");
    // A star cost shows after the energy cost.
    let state = app.combat().expect("a fight").state();
    if state.player.hand.iter().any(|&c| state.star_cost(c) >= 0) {
        assert!(text.contains("·s"));
    }
}

#[test]
fn the_necrobinder_shows_osty() {
    let app = first_fight(Character::Necrobinder, "RL2S2");
    assert!(render(&app).contains("Osty"));
}

#[test]
fn the_pile_overlays_show_their_cards_and_the_draw_pile_is_sorted() {
    for character in Character::ALL {
        let mut app = first_fight(character, "RL2S2");
        let counts = {
            let p = &app.combat().expect("a fight").state().player;
            (p.draw.len(), p.discard.len(), p.exhaust.len())
        };
        app.handle_key(KeyCode::Char('a'));
        let draw = render(&app);
        assert!(
            draw.contains(&format!("Draw pile ({})", counts.0)),
            "{character:?}"
        );
        assert!(draw.contains("order is hidden"));
        app.handle_key(KeyCode::Char('s'));
        assert!(render(&app).contains(&format!("Discard pile ({})", counts.1)));
        app.handle_key(KeyCode::Char('x'));
        let exhaust = render(&app);
        assert!(exhaust.contains(&format!("Exhaust pile ({})", counts.2)));
        assert!(exhaust.contains("(empty)"));
        app.handle_key(KeyCode::Char('d'));
        assert!(
            render(&app).contains("Deck (10 cards)") || render(&app).contains("Deck (12 cards)")
        );
        app.handle_key(KeyCode::Char('v'));
        assert!(render(&app).contains("Relics"));
        app.handle_key(KeyCode::Char('i'));
        assert!(render(&app).contains("Info"));
        app.handle_key(KeyCode::Char('m'));
        let map = render(&app);
        assert!(
            map.contains("Act 1 map") && map.contains('●'),
            "{character:?}"
        );
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.overlay(), None);
    }
}

#[test]
fn the_draw_pile_is_listed_as_counts_in_alphabetical_order_not_draw_order() {
    let mut app = first_fight(Character::Ironclad, "RL2S2");
    app.handle_key(KeyCode::Char('a'));
    // The overlay's card lines are `[Nx ]Name (Type, N energy)`, in the column under its title.
    let mut terminal = Terminal::new(TestBackend::new(110, 42)).expect("test backend");
    terminal.draw(|f| ui::draw(f, &app)).expect("draw");
    let buf = terminal.backend().buffer();
    let cells: Vec<Vec<&str>> = (0..42)
        .map(|y| (0..110).map(|x| buf[(x, y)].symbol()).collect())
        .collect();
    let title = cells
        .iter()
        .position(|row| row.join("").contains("Draw pile"))
        .expect("the popup");
    let listed: Vec<(usize, String)> = cells[title + 2..]
        .iter()
        .filter_map(|row| {
            let line = row.concat();
            let (name, rest) = line.trim().split_once(" (")?;
            let (count, name) = name
                .split_once("x ")
                .and_then(|(n, name)| Some((n.parse().ok()?, name)))
                .unwrap_or((1, name));
            rest.contains("energy)").then(|| (count, name.to_owned()))
        })
        .collect();
    let names: Vec<&String> = listed.iter().map(|(_, name)| name).collect();
    assert!(names.len() >= 2, "{listed:?}");
    assert!(names.is_sorted(), "{listed:?}");
    // Identical cards share a line: every name once, the counts add up to the pile.
    assert!(names.windows(2).all(|w| w[0] != w[1]), "{listed:?}");
    let draw = app.combat().expect("a fight").state().player.draw.len();
    assert_eq!(listed.iter().map(|(n, _)| n).sum::<usize>(), draw);
}

#[test]
fn piles_exist_only_in_a_fight() {
    let mut app = plain_app("RL2S2");
    app.handle_key(KeyCode::Char('a'));
    assert_eq!(app.overlay(), None);
    assert_eq!(app.error(), Some("the piles exist only in a fight"));
    // The full map is there on the Neow screen too.
    app.handle_key(KeyCode::Char('m'));
    assert!(render(&app).contains("Act 1 map"));
}

#[test]
fn small_and_large_terminals_do_not_break_the_screens() {
    for character in Character::ALL {
        let mut app = first_fight(character, "RL2S2");
        for (w, h) in [
            (40, 12),
            (60, 20),
            (80, 24),
            (100, 30),
            (160, 50),
            (200, 60),
        ] {
            render_at(&app, w, h);
        }
        app.handle_key(KeyCode::Char('d'));
        render_at(&app, 50, 15);
        app.handle_key(KeyCode::Esc);
        // A target prompt highlights the card and the enemies.
        app.handle_key(KeyCode::Char('e'));
        render_at(&app, 90, 30);
    }
}

#[test]
fn the_act_comes_from_the_seed_not_from_the_screen() {
    // `App::new` rolls Act 1 from the seed as the game does: both acts show up.
    let acts: BTreeSet<&str> = (0..40)
        .map(|n| {
            let app = App::with_text(
                Character::Ironclad,
                &format!("RL2S{n}"),
                0,
                Rc::new(Localization::empty()),
            )
            .expect("run");
            app.run().state().act().act.id
        })
        .collect();
    assert!(
        acts.contains("OVERGROWTH") && acts.contains("UNDERDOCKS"),
        "{acts:?}"
    );
}

#[test]
fn the_select_screen_lists_every_character_the_ascension_and_the_seed() {
    let mut shell = Shell::select(Rc::new(Localization::empty()));
    let draw = |shell: &Shell| {
        let mut terminal = Terminal::new(TestBackend::new(110, 42)).expect("test backend");
        terminal.draw(|f| ui::draw_shell(f, shell)).expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(Cell::symbol)
            .collect::<String>()
    };
    // One character at a time: each number shows its own, with its starting relic.
    for (i, character) in sts2_tui::shell::SELECT_ORDER.into_iter().enumerate() {
        shell.handle_key(KeyCode::Char(char::from(
            b'1' + u8::try_from(i).expect("small"),
        )));
        let text = draw(&shell);
        assert!(
            text.contains(&character.name().to_uppercase()),
            "no {character:?}"
        );
        assert!(
            text.contains(character.starting_relic().name()),
            "{character:?}'s relic"
        );
    }
    let text = draw(&shell);
    for part in ["Ascension 0", "Seed", "random", "Start"] {
        assert!(text.contains(part), "no {part}");
    }
    assert!(!text.contains("Act 1 choice") && !text.contains("Underdocks"));
    shell.handle_key(KeyCode::Char('3'));
    for _ in 0..3 {
        shell.handle_key(KeyCode::Up);
    }
    shell.handle_key(KeyCode::Char('s'));
    for c in "my1".chars() {
        shell.handle_key(KeyCode::Char(c));
    }
    let text = draw(&shell);
    assert!(text.contains("Ascension 3") && text.contains("MY1▌"));
    let regent = sts2_tui::shell::SELECT_ORDER[2];
    assert!(text.contains(regent.starting_relic().name()));
}
