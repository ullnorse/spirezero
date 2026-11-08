//! Potions, keyboard first: one key selects a potion; one with one living enemy to hit is
//! thrown at once, one with more waits for its target (arrows and Enter, a number, Esc), one
//! that needs no target asks in place (Enter or the key again uses it, `t` throws it away,
//! anything else puts it back); the `≡` menu's throw-away rows ask in place; a potion the
//! engine allows outside a fight works there too. The mouse: a click selects, a click on an
//! enemy aims, a second click uses.

use ratatui::crossterm::event::KeyCode;
use sts2_core::combat::CreatureId;
use sts2_core::run::RunDecision;
use sts2_tui::app::{App, Input, Overlay, Pending};
use sts2_tui::hit::Target;
use sts2_tui::keys;

use crate::snapshots::{fight, fight_with_deck, map_app_with_potions, press, render};

fn hp(app: &App, id: CreatureId) -> i32 {
    app.combat().expect("fight").state().creature(id).hp
}

fn block(app: &App) -> i32 {
    let state = app.combat().expect("fight").state();
    state.creature(state.player.creature).block
}

fn enemies(app: &App) -> Vec<CreatureId> {
    app.combat().expect("fight").state().enemies.clone()
}

fn living(app: &App) -> Vec<CreatureId> {
    let state = app.combat().expect("fight").state();
    state
        .enemies
        .iter()
        .copied()
        .filter(|&e| state.creature(e).is_alive())
        .collect()
}

fn hand_len(app: &App) -> usize {
    app.combat().expect("fight").state().player.hand.len()
}

/// The potion row prints each potion's key, in a fight even with an empty belt.
#[test]
fn the_potion_row_is_always_on_the_fight_screen_with_its_keys() {
    let screen = render(&fight(), 80, 24);
    assert!(screen.contains("z ◈ Fire   c ◈ Block"), "{screen}");
    assert_eq!(keys::potion_key(0), 'z');
    assert_eq!(keys::potion_key(1), 'c');
    let mut empty = map_app_with_potions(&[None, None, None]);
    assert!(
        !render(&empty, 80, 24).contains("No potions"),
        "outside a fight an empty belt takes no row"
    );
    let first = empty
        .run()
        .legal_actions()
        .into_iter()
        .find_map(|a| match a {
            sts2_core::run::RunAction::ChooseNode(c) => Some(c),
            _ => None,
        })
        .expect("a node");
    empty.perform(sts2_tui::hit::UiAction::Travel(first));
    assert!(empty.combat().is_some());
    assert!(
        render(&empty, 80, 24).contains("No potions"),
        "in a fight the row stays"
    );
    press(&mut empty, "z");
    assert_eq!(empty.error(), Some("no potion in slot 1"));
}

/// A targeted potion with several living enemies waits for its target: a number, or the
/// arrows and Enter; Esc puts it back.
#[test]
fn one_key_selects_a_potion_and_its_target_is_picked_or_cancelled() {
    let mut app = fight();
    let foes = enemies(&app);
    assert_eq!(foes.len(), 3);
    press(&mut app, "z");
    assert_eq!(app.input(), &Input::Target(Pending::Potion(0)));
    assert!(
        render(&app, 80, 24).contains("Fire Potion → 1-3, or ←→ Enter · Esc"),
        "{}",
        render(&app, 80, 24)
    );
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.input(), &Input::Idle, "Esc puts it back");
    assert_eq!(app.run().potions()[0], Some("FIRE_POTION"));

    // The arrows walk the targets; Enter throws.
    press(&mut app, "z");
    let focus = app.focused();
    app.handle_key(KeyCode::Right);
    assert_ne!(app.focused(), focus, "→ moves to the next target");
    let Some(Target::Creature(aimed)) = app.focused() else {
        panic!("a target is focused: {:?}", app.focused());
    };
    let before = hp(&app, aimed);
    app.handle_key(KeyCode::Enter);
    assert!(hp(&app, aimed) < before, "the potion hit the focused enemy");
    assert_eq!(app.run().potions()[0], None, "the potion is gone");
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(hand_len(&app), 5, "no card was played");
}

/// A number picks the target of a waiting potion.
#[test]
fn a_number_targets_a_waiting_potion() {
    let mut app = fight();
    let foes = enemies(&app);
    let before = hp(&app, foes[1]);
    press(&mut app, "z2");
    assert!(hp(&app, foes[1]) < before, "enemy 2 was hit");
    assert_eq!(app.run().potions()[0], None);
}

/// With one living enemy a targeted potion is thrown at once: no targeting step.
#[test]
fn a_potion_with_one_living_enemy_is_thrown_at_once() {
    let mut app = fight_with_deck(&["STRIKE_IRONCLAD"; 10]);
    let foes = enemies(&app);
    // Strikes at the two smaller slimes until only the middle one is left, ending turns as
    // the energy runs out.
    for _ in 0..60 {
        let Some(victim) = living(&app).into_iter().find(|&e| e != foes[1]) else {
            break;
        };
        // The enemies' numbers follow the engine's list, which drops the dead.
        let at = enemies(&app)
            .iter()
            .position(|&e| e == victim)
            .expect("an enemy")
            + 1;
        let state = app.combat().expect("fight").state();
        let strike = state
            .player
            .hand
            .iter()
            .position(|&c| sts2_tui::names::card(state, c) == "Strike");
        match strike {
            Some(i) => press(&mut app, &format!("{}{at}", i + 1)),
            None => press(&mut app, "e"),
        }
        if app.error().is_some() || !matches!(app.input(), Input::Idle) {
            app.handle_key(KeyCode::Esc);
            press(&mut app, "e");
        }
    }
    assert_eq!(
        living(&app),
        vec![foes[1]],
        "one enemy left: {:?}",
        living(&app)
    );
    let before = hp(&app, foes[1]);
    press(&mut app, "z");
    assert_eq!(app.input(), &Input::Idle, "thrown at once");
    assert!(hp(&app, foes[1]) < before, "the potion hit the one enemy");
    assert_eq!(app.run().potions()[0], None);
}

/// A potion that needs no target asks in place on its row: Enter or its key again uses it,
/// Esc or any other key puts it back without doing anything else.
#[test]
fn a_potion_without_a_target_asks_in_place() {
    let mut app = fight();
    press(&mut app, "c");
    assert_eq!(app.input(), &Input::Potion(1));
    assert_eq!(
        app.focused(),
        Some(Target::Potion(1)),
        "the row is highlighted"
    );
    let screen = render(&app, 120, 35);
    assert!(
        screen.contains("c ◈ Block Potion?  Enter use · t throw away · Esc"),
        "{screen}"
    );
    let narrow = render(&app, 80, 24);
    assert!(
        narrow.contains("c ◈ Block Potion?  Enter use · t throw away") && !narrow.contains("· Esc"),
        "the last hint left out where the row is narrow\n{narrow}"
    );
    assert_eq!(block(&app), 0);
    // Any other key puts it back and does nothing else.
    press(&mut app, "1");
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(hand_len(&app), 5, "the card was not played");
    assert_eq!(app.run().potions()[1], Some("BLOCK_POTION"));
    // Esc puts it back.
    press(&mut app, "c");
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(app.run().potions()[1], Some("BLOCK_POTION"));
    // The key again uses it.
    press(&mut app, "cc");
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(app.run().potions()[1], None, "used");
    assert!(block(&app) > 0, "the Block Potion gave Block");
    assert_eq!(app.error(), None);
}

/// Enter on the selected potion uses it too; another potion's key selects that one instead.
#[test]
fn enter_uses_the_selected_potion_and_another_key_selects_another() {
    let mut app = fight();
    press(&mut app, "c");
    press(&mut app, "z");
    assert_eq!(
        app.input(),
        &Input::Target(Pending::Potion(0)),
        "the Fire Potion is selected instead"
    );
    app.handle_key(KeyCode::Esc);
    press(&mut app, "c");
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.run().potions()[1], None, "used");
    assert!(block(&app) > 0);
}

/// `t` on a selected potion throws it away; the `≡` menu's row asks in place and its number
/// again throws it away.
#[test]
fn a_potion_is_thrown_away_from_its_row_or_the_menu() {
    let mut app = fight();
    press(&mut app, "ct");
    assert_eq!(app.run().potions()[1], None, "thrown away");
    assert_eq!(block(&app), 0, "not used");
    assert_eq!(app.input(), &Input::Idle);
    // From the menu: `1` asks, `1` again throws the Fire Potion away.
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.overlay(), Some(Overlay::Menu));
    let screen = render(&app, 80, 30);
    assert!(screen.contains(" 1  Throw away Fire Potion"), "{screen}");
    press(&mut app, "1");
    let screen = render(&app, 80, 30);
    assert!(
        screen.contains("Throw away Fire Potion? press 1"),
        "{screen}"
    );
    assert_eq!(app.run().potions()[0], Some("FIRE_POTION"), "asked first");
    // Another key puts the question back.
    press(&mut app, "d");
    assert_eq!(app.run().potions()[0], Some("FIRE_POTION"));
    assert_eq!(app.overlay(), Some(Overlay::Menu), "the menu stays");
    press(&mut app, "11");
    assert_eq!(app.run().potions()[0], None, "thrown away");
    assert_eq!(app.overlay(), None, "the menu closed on it");
    // No potion left: the row says so, and the keys say why.
    assert!(render(&app, 80, 24).contains("No potions"));
    press(&mut app, "z");
    assert_eq!(app.error(), Some("no potion in slot 1"));
}

/// A potion the engine allows outside a fight (the Blood Potion heals) works on the map with
/// the same key and the same question; one it does not allow says so.
#[test]
fn a_potion_is_used_outside_a_fight_where_the_engine_allows_it() {
    let mut app = map_app_with_potions(&[Some("BLOOD_POTION"), Some("FIRE_POTION"), None]);
    assert!(matches!(app.run().pending_decision(), RunDecision::Map(_)));
    let before = app.run().state().hp;
    assert_eq!(before, 60);
    press(&mut app, "z");
    assert_eq!(app.input(), &Input::Potion(0), "{:?}", app.error());
    assert!(render(&app, 80, 24).contains("z ◈ Blood Potion?  Enter use"));
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.run().potions()[0], None, "used");
    assert!(app.run().state().hp > before, "healed");
    press(&mut app, "c");
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(app.error(), Some("Fire Potion cannot be used now"));
}
