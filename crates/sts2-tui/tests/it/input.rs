//! The keys of a fight and the menus: the arrows and Enter on the highlighted choice, a card's
//! number then its target's, one living enemy targeted at once, Backspace or the card's own
//! number cancelling, Space aiming at the last target again, `i` then a number inspecting,
//! `j`/`k` scrolling, `s` skipping a card reward, `l` and `r` in the shop.

use ratatui::crossterm::event::KeyCode;
use sts2_core::Character;
use sts2_core::combat::CreatureId;
use sts2_core::run::{Act1, Reward, Run, RunAction, RunDecision, UnlockState};
use sts2_tui::app::{App, Input, Inspect, Overlay, Pending};
use sts2_tui::hit::{Target, UiAction};
use sts2_tui::shell::{Mode, Shell};

use crate::snapshots::{fight, fight_with_deck, map_app, press, render, rewards, shop, win};

fn hp(app: &App, id: CreatureId) -> i32 {
    app.combat().expect("fight").state().creature(id).hp
}

fn enemies(app: &App) -> Vec<CreatureId> {
    app.combat().expect("fight").state().enemies.clone()
}

fn hand_len(app: &App) -> usize {
    app.combat().expect("fight").state().player.hand.len()
}

#[test]
fn a_card_then_its_target_plays_it_and_backspace_or_its_own_number_cancels() {
    let mut app = fight();
    let foes = enemies(&app);
    assert_eq!(
        foes.len(),
        3,
        "the first fight of the seed has three enemies"
    );
    // The hand starts Strike, Defend, Defend, Strike, Defend.
    press(&mut app, "1");
    assert!(matches!(app.input(), Input::Target(Pending::Card(_))));
    app.handle_key(KeyCode::Backspace);
    assert_eq!(app.input(), &Input::Idle, "Backspace cancels");
    press(&mut app, "4");
    assert!(matches!(app.input(), Input::Target(Pending::Card(_))));
    press(&mut app, "4");
    assert_eq!(app.input(), &Input::Idle, "the card's own number cancels");
    assert_eq!(app.error(), None);
    assert_eq!(hand_len(&app), 5, "nothing was played");

    let before = hp(&app, foes[1]);
    press(&mut app, "12");
    assert!(hp(&app, foes[1]) < before, "card 1 hit enemy 2");
    assert_eq!(app.last_target(), Some(foes[1]));
    assert_eq!(hand_len(&app), 4);
}

#[test]
fn space_aims_at_the_last_target_again() {
    let mut app = fight();
    let foes = enemies(&app);
    press(&mut app, "1");
    app.handle_key(KeyCode::Char(' '));
    assert_eq!(
        app.error(),
        Some("no target used yet this fight"),
        "nothing aimed yet"
    );
    assert!(matches!(app.input(), Input::Target(_)), "still waiting");
    press(&mut app, "3");
    let after_one = hp(&app, foes[2]);
    // Strike is now the third card of four: Defend, Defend, Strike, Defend.
    press(&mut app, "3");
    assert!(matches!(app.input(), Input::Target(_)));
    app.handle_key(KeyCode::Char(' '));
    assert!(
        hp(&app, foes[2]) < after_one
            || !app
                .combat()
                .expect("fight")
                .state()
                .creature(foes[2])
                .is_alive(),
        "Space played it at enemy 3 again"
    );
    assert_eq!(hand_len(&app), 3);
}

/// The first fight of seed `RL2S2` as the Ironclad: one Nibbit.
fn single_enemy_fight() -> App {
    let mut run = Run::new(
        Character::Ironclad,
        "RL2S2",
        0,
        UnlockState::all(),
        Act1::Random,
    )
    .expect("run")
    .with_events();
    for _ in 0..50 {
        if run.combat().is_some() {
            break;
        }
        let next = run
            .legal_actions()
            .into_iter()
            .find(|a| run.clone().apply(a.clone()).is_ok())
            .expect("a way on");
        run.apply(next).expect("legal");
    }
    App::with_run("RL2S2", 0, run)
}

#[test]
fn one_living_enemy_is_targeted_at_once() {
    let mut app = single_enemy_fight();
    let foes = enemies(&app);
    assert_eq!(foes.len(), 1);
    let state = app.combat().expect("fight").state();
    let attack = state
        .player
        .hand
        .iter()
        .position(|&c| {
            app.run()
                .legal_actions()
                .contains(&RunAction::Combat(sts2_core::Action::PlayCard {
                    card: c,
                    target: Some(foes[0]),
                }))
        })
        .expect("an attack in hand");
    let before = hp(&app, foes[0]);
    press(&mut app, &(attack + 1).to_string());
    assert_eq!(app.input(), &Input::Idle, "no second key");
    assert!(hp(&app, foes[0]) < before, "played at the only enemy");
}

#[test]
fn i_then_a_number_inspects_a_card_or_an_enemy() {
    let mut app = fight();
    let state = app.combat().expect("fight").state();
    let (second, third_enemy) = (state.player.hand[1], state.enemies[2]);
    press(&mut app, "i");
    assert_eq!(
        app.overlay(),
        Some(Overlay::Info),
        "the info until a number"
    );
    press(&mut app, "2");
    assert_eq!(app.overlay(), Some(Overlay::Inspect(Inspect::Card(second))));
    press(&mut app, "i");
    assert_eq!(app.overlay(), None, "i closes it");
    press(&mut app, "ie3");
    assert_eq!(
        app.overlay(),
        Some(Overlay::Inspect(Inspect::Enemy(third_enemy)))
    );
    app.handle_key(KeyCode::Backspace);
    assert_eq!(app.overlay(), None, "Backspace closes it");
    assert_eq!(hand_len(&app), 5, "nothing was played");
}

#[test]
fn j_and_k_scroll_the_log() {
    let mut app = fight();
    press(&mut app, "e");
    // The log is drawn once so it knows how far it can scroll.
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 35)).expect("backend");
    terminal
        .draw(|f| sts2_tui::ui::draw(f, &app))
        .expect("draw");
    press(&mut app, "kk");
    assert_eq!(app.log_scroll(), 2);
    press(&mut app, "j");
    assert_eq!(app.log_scroll(), 1);
}

#[test]
fn s_skips_the_card_reward() {
    let mut app = rewards();
    let has_card = |app: &App| match app.run().pending_decision() {
        RunDecision::Rewards(rewards) => rewards.iter().any(|r| matches!(r, Reward::Card(_))),
        _ => false,
    };
    assert!(has_card(&app));
    // The card reward is its own screen: its row opens it, `s` there skips it.
    let row = app
        .menu()
        .iter()
        .filter(|i| i.enabled)
        .position(|i| i.open.is_some())
        .expect("the card reward's row");
    press(&mut app, &(row + 1).to_string());
    assert!(app.open_reward().is_some(), "the card reward opens");
    assert!(has_card(&app));
    press(&mut app, "s");
    assert_eq!(app.error(), None);
    assert!(has_card(&app), "Skip asks in place first");
    assert!(app.confirm_prompt().is_some_and(|p| p.starts_with("Skip")));
    press(&mut app, "s");
    assert!(!has_card(&app), "s again skips the card reward");
    assert_eq!(app.open_reward(), None);
}

#[test]
fn the_shop_has_no_letters_of_its_own_and_esc_leaves() {
    let mut app = shop();
    press(&mut app, "r");
    assert_eq!(app.error(), Some("no such key · ? lists the keys"));
    assert!(matches!(app.run().pending_decision(), RunDecision::Shop(_)));
    let mut app = shop();
    app.handle_key(KeyCode::Esc);
    if app.confirm_prompt().is_some() {
        app.handle_key(KeyCode::Enter);
    }
    assert!(
        matches!(app.run().pending_decision(), RunDecision::Map(_)),
        "l leaves"
    );
    // An open list takes the keys: `s` skips nothing under it.
    let mut app = fight();
    win(&mut app);
    app.perform(UiAction::Toggle(Overlay::Deck));
    press(&mut app, "s");
    assert_eq!(
        app.overlay(),
        Some(Overlay::Deck),
        "an open list takes the keys"
    );
}

fn block(app: &App) -> i32 {
    let state = app.combat().expect("fight").state();
    state.creature(state.player.creature).block
}

#[test]
fn neow_is_answered_with_the_arrows_and_enter() {
    let mut app = App::new(Character::Ironclad, "RL2S2", 0).expect("run");
    assert!(matches!(
        app.run().pending_decision(),
        RunDecision::Ancient(_)
    ));
    assert_eq!(
        app.focused(),
        Some(Target::Button(UiAction::Pick(1))),
        "the first option starts highlighted"
    );
    app.handle_key(KeyCode::Down);
    assert_eq!(app.focused(), Some(Target::Button(UiAction::Pick(2))));
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Up);
    let n = app.menu().iter().filter(|i| i.enabled).count();
    assert_eq!(
        app.focused(),
        Some(Target::Button(UiAction::Pick(n))),
        "wraps round to the last"
    );
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Down);
    let mut by_number = app.clone();
    press(&mut by_number, "2");
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.error(), None);
    assert_eq!(app.run(), by_number.run(), "Enter picked option 2");
    assert!(!matches!(
        app.run().pending_decision(),
        RunDecision::Ancient(_)
    ));
}

#[test]
fn the_map_is_travelled_with_the_arrows_and_enter() {
    let mut app = map_app();
    let RunDecision::Map(nodes) = app.run().pending_decision() else {
        panic!("on the map");
    };
    let nodes: Vec<_> = nodes.iter().copied().collect();
    assert!(nodes.len() >= 2, "a choice of rooms: {nodes:?}");
    assert_eq!(app.focused(), Some(Target::Button(UiAction::Pick(1))));
    app.handle_key(KeyCode::Right);
    assert_eq!(app.focused(), Some(Target::Button(UiAction::Pick(2))));
    app.handle_key(KeyCode::Left);
    app.handle_key(KeyCode::Right);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.error(), None);
    assert_eq!(
        app.run().state().coord(),
        Some(nodes[1]),
        "travelled to the second room"
    );
}

#[test]
fn one_key_plays_a_card_that_needs_no_target() {
    let mut app = fight();
    // The hand starts Strike, Defend, Defend, Strike, Defend.
    let before = block(&app);
    press(&mut app, "2");
    assert_eq!(app.input(), &Input::Idle, "no second press");
    assert_eq!(hand_len(&app), 4, "Defend was played");
    assert!(block(&app) > before);
    // The same by Enter: the focus moves down one row, onto a Defend again.
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Enter);
    assert_eq!(hand_len(&app), 3);
    assert_eq!(app.input(), &Input::Idle);
}

#[test]
fn enter_plays_the_highlighted_card_and_the_arrows_pick_its_target() {
    let mut app = fight();
    let foes = enemies(&app);
    let hand = app.combat().expect("fight").state().player.hand.clone();
    assert_eq!(
        app.focused(),
        Some(Target::Card(hand[0])),
        "the first playable card starts highlighted"
    );
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.input(), &Input::Target(Pending::Card(hand[0])));
    assert_eq!(
        app.focused(),
        Some(Target::Creature(foes[0])),
        "the first enemy, no target used yet"
    );
    app.handle_key(KeyCode::Right);
    assert_eq!(app.focused(), Some(Target::Creature(foes[1])));
    let before = hp(&app, foes[1]);
    app.handle_key(KeyCode::Enter);
    assert!(hp(&app, foes[1]) < before, "Strike hit enemy 2");
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(hand_len(&app), 4);
    assert_eq!(
        app.focused(),
        Some(Target::Card(hand[1])),
        "the focus stays on the row the played card had"
    );
    // Esc puts a card waiting for its target back; the next aim starts on the last target.
    press(&mut app, "3");
    assert!(matches!(app.input(), Input::Target(_)));
    assert_eq!(app.focused(), Some(Target::Creature(foes[1])));
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.input(), &Input::Idle);
    assert_eq!(hand_len(&app), 4, "nothing played");
}

#[test]
fn end_turn_and_the_potions_are_reached_with_the_arrows() {
    let mut app = fight();
    let round = app.combat().expect("fight").state().round;
    app.handle_key(KeyCode::Up);
    assert_eq!(
        app.focused(),
        Some(Target::Potion(1)),
        "above the first card: the last potion"
    );
    // Enter selects it and asks in place (the Block Potion needs no target); Enter uses it.
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.input(), &Input::Potion(1), "{:?}", app.error());
    assert_eq!(app.run().potions()[1], Some("BLOCK_POTION"), "not yet");
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.run().potions()[1], None, "used");
    assert_eq!(app.input(), &Input::Idle);
    // Down past the last card: End Turn.
    for _ in 0..hand_len(&app) {
        app.handle_key(KeyCode::Down);
    }
    assert_eq!(app.focused(), Some(Target::Button(UiAction::EndTurn)));
    app.handle_key(KeyCode::Enter);
    let after = app.combat().map(|c| c.state().round);
    assert!(after.is_none_or(|r| r > round), "the turn ended");
    if app.combat().is_some() {
        assert!(
            matches!(app.focused(), Some(Target::Card(_))),
            "a new turn starts on a card again: {:?}",
            app.focused()
        );
    }
}

#[test]
fn the_menu_overlay_is_navigated_with_the_arrows() {
    let mut app = fight();
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.overlay(), Some(Overlay::Menu));
    assert_eq!(
        app.focused(),
        Some(Target::Button(UiAction::Toggle(Overlay::Deck)))
    );
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.overlay(), Some(Overlay::Relics), "the second entry");
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.overlay(), None);
    assert!(matches!(app.focused(), Some(Target::Card(_))));
}

/// Playing the last card with energy left keeps the focus in the hand, on the card now last.
#[test]
fn playing_the_last_card_with_energy_left_keeps_the_focus_on_a_card() {
    let mut app = fight();
    // The hand starts Strike, Defend, Defend, Strike, Defend.
    let hand = app.combat().expect("fight").state().player.hand.clone();
    for _ in 0..4 {
        app.handle_key(KeyCode::Down);
    }
    assert_eq!(app.focused(), Some(Target::Card(hand[4])));
    app.handle_key(KeyCode::Enter);
    assert_eq!(hand_len(&app), 4, "the last Defend was played");
    assert_eq!(
        app.focused(),
        Some(Target::Card(hand[3])),
        "the card now last, not End Turn: there is energy for it"
    );
}

/// With the energy spent and only cost-1 cards left, the focus moves to End Turn.
#[test]
fn spending_all_energy_with_only_costly_cards_left_focuses_end_turn() {
    let mut app = fight();
    let hand = app.combat().expect("fight").state().player.hand.clone();
    for _ in 0..4 {
        app.handle_key(KeyCode::Down);
    }
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focused(), Some(Target::Card(hand[3])), "a Strike");
    press(&mut app, "2");
    assert_eq!(app.focused(), Some(Target::Card(hand[3])), "still there");
    press(&mut app, "2");
    assert_eq!(hand_len(&app), 2, "two Strikes, no energy");
    assert_eq!(
        app.focused(),
        Some(Target::Button(UiAction::EndTurn)),
        "no card can be played"
    );
}

/// A 0-cost card can still be played without energy: the focus rests on it, not End Turn.
#[test]
fn a_zero_cost_card_keeps_the_focus_in_the_hand_without_energy() {
    let mut app = fight_with_deck(&[
        "STRIKE_IRONCLAD",
        "STRIKE_IRONCLAD",
        "STRIKE_IRONCLAD",
        "ANGER",
        "DEFEND_IRONCLAD",
    ]);
    let state = app.combat().expect("fight").state();
    let anger = state
        .player
        .hand
        .iter()
        .copied()
        .find(|&c| state.card(c).kind.name() == "Anger")
        .expect("Anger in hand");
    for _ in 0..3 {
        let state = app.combat().expect("fight").state();
        let strike = state
            .player
            .hand
            .iter()
            .position(|&c| state.card(c).kind.name() == "Strike")
            .expect("a Strike");
        press(&mut app, &format!("{}1", strike + 1));
    }
    assert_eq!(app.combat().expect("fight").state().player.energy, 0);
    assert_eq!(app.focused(), Some(Target::Card(anger)), "the 0-cost card");
    // An explicit focus on it, then its play: nothing playable is left.
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Up);
    assert_eq!(app.focused(), Some(Target::Card(anger)));
    press(&mut app, "1");
    assert!(
        matches!(app.input(), Input::Target(_)),
        "Anger aims at an enemy"
    );
    press(&mut app, "1");
    assert_eq!(hand_len(&app), 1, "Defend alone is left");
    assert_eq!(
        app.focused(),
        Some(Target::Button(UiAction::EndTurn)),
        "Defend cannot be played without energy"
    );
}

/// The shell playing `app`, waiting (a minute at most) for the bot's answer to `w`.
fn ask_the_bot(app: App) -> Shell {
    let mut shell = Shell::play(app);
    shell.handle_key(KeyCode::Char('w'));
    let started = std::time::Instant::now();
    while shell.thinking() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "the bot answers"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        shell.poll_hint();
    }
    shell
}

fn hint_of(shell: &Shell) -> Option<String> {
    match shell.mode() {
        Mode::Play(app) => app.hint().map(str::to_owned),
        Mode::Select(_) => None,
    }
}

#[test]
fn w_shows_the_bots_advice_on_the_map_and_in_a_fight_and_it_goes_with_the_next_action() {
    let shell = ask_the_bot(map_app());
    let hint = hint_of(&shell).expect("advice on the map");
    assert!(
        hint.starts_with("Bot: 1 ") || hint.starts_with("Bot: 2 ") || hint.starts_with("Bot: 3 "),
        "it names the row to press: {hint}"
    );
    assert!(hint.ends_with("sure)"), "and how sure it is: {hint}");

    let mut shell = ask_the_bot(fight());
    let hint = hint_of(&shell).expect("advice in a fight");
    assert!(
        hint.starts_with("Bot: play ") || hint.starts_with("Bot: end the turn"),
        "{hint}"
    );
    assert!(hint.contains("Win chance"), "{hint}");
    // The key is `w` anywhere in a run and the advice shows in the frame.
    let Mode::Play(app) = shell.mode() else {
        panic!("a run");
    };
    assert!(render(app, 80, 24).contains("Bot: "));
    // Ending the turn is an action: the advice for the old position is gone.
    shell.handle_key(KeyCode::Char('e'));
    assert_eq!(hint_of(&shell), None);
}
