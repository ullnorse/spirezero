//! Plays whole runs, Neow to the victory screen, through the key handler only: a seeded policy
//! picks among the engine's legal actions and presses the keys that make them. Every action the
//! engine lists outside a fight must have a key path (a menu entry or the potion keys), every
//! screen is rendered, and no decision may leave the TUI without a way to answer.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::KeyCode;
use sts2_core::combat::Decision;
use sts2_core::content::cards::TargetType;
use sts2_core::content::events::EventKind;
use sts2_core::run::RoomType;
use sts2_core::run::{Act1, Reward, Room, Run, RunAction, RunDecision, RunError, UnlockState};
use sts2_core::{Action, Character, Status};
use sts2_tui::app::{App, Input};
use sts2_tui::hit::Target;
use sts2_tui::keys;
use sts2_tui::text::Localization;
use sts2_tui::ui;

/// Actions pressed before a run is given up as stuck.
const MAX_ACTIONS: usize = 60_000;
/// Card plays in one turn before the policy ends it (zero-cost loops).
const MAX_PLAYS_PER_TURN: usize = 40;
/// HP given at the first map when a run is made safe.
const SAFE_HP: i32 = 9_999;

/// A small deterministic generator for the policy's choices.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % n as u64).expect("small")
    }
}

/// An accepted action changes the run. Trial's `DoubleDown` is the exception: it only opens the
/// abandon-run popup and the same page stays up (`disableOnChosen` false).
fn assert_changed(run: &Run, before: &Run, action: &RunAction) {
    let reopens_its_page = match (before.pending_decision(), action) {
        (RunDecision::Event(room), RunAction::EventOption(i)) => room
            .options
            .get(*i)
            .is_some_and(|o| o.text_key.ends_with(".DOUBLE_DOWN")),
        _ => false,
    };
    assert!(
        run != before || reopens_its_page,
        "{action:?} changed nothing"
    );
}

fn render(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
    terminal.draw(|f| ui::draw(f, app)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(Cell::symbol)
        .collect::<Vec<_>>()
        .chunks(usize::from(width))
        .map(|row| row.concat().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
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

/// Whether `action` is reachable by keys: on the menu, behind the card reward's row (its
/// screen holds its answers), on the potion keys, or on Esc (cancelling a selection; the way
/// back from a card reward's screen to the other rewards).
fn has_key_path(app: &App, action: &RunAction) -> bool {
    let menu = app.menu();
    let on_menu = menu.iter().any(|i| i.enabled && i.action == *action);
    let behind_a_row = card_reward_of(app, action)
        .is_some_and(|i| menu.iter().any(|item| item.enabled && item.open == Some(i)));
    let on_potion_keys = matches!(
        action,
        RunAction::UsePotion(_) | RunAction::DiscardPotion(_)
    );
    let on_esc = *action == RunAction::CancelSelect
        || (app.open_reward().is_some() && card_reward_of(app, action).is_none())
        || app.open_section().is_some();
    let behind_a_section = sts2_view::menu::buy_section(app.run(), action).is_some_and(|s| {
        menu.iter()
            .any(|i| i.enabled && i.section.map(|(x, _)| x) == Some(s))
    });
    on_menu || behind_a_section || behind_a_row || on_potion_keys || on_esc
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

/// The screen kind and a text its render must contain.
fn screen(run: &Run) -> (&'static str, &'static str) {
    match run.pending_decision() {
        RunDecision::Ancient(_) => ("ancient", "Choose one"),
        RunDecision::Event(_) => ("event", "1  "),
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

/// Renders the screen and checks the HUD and the screen's own text are there.
fn smoke(app: &App, seen: &mut BTreeSet<&'static str>) {
    let (kind, needle) = screen(app.run());
    let text = render(app, 120, 44);
    for header in ["♥", "◉", "Floor", "≡"] {
        assert!(text.contains(header), "{kind}: HUD lacks {header}");
    }
    // A card reward open as its own screen has its own title.
    // A shop section open is the shop's second level, titled by the section.
    let section = kind == "shop" && app.open_section().is_some();
    let found =
        text.contains(needle) || (kind == "rewards" && text.contains("Choose a card")) || section;
    assert!(found, "{kind}: screen lacks {needle:?}\n{text}");
    seen.insert(kind);
}

/// How a run ended.
#[derive(Debug)]
struct Outcome {
    end: &'static str,
    screens: BTreeSet<&'static str>,
    /// Rejections for content the engine does not port, shown as errors by the TUI.
    unported: BTreeSet<String>,
    acts: usize,
    /// The encounter of the last fight when the run ended.
    killer: String,
    /// The (act index, encounter) of every boss fight started.
    bosses: BTreeSet<(usize, String)>,
    /// The kinds of action the policy took.
    actions: BTreeSet<String>,
}

/// The keys that make `action`. Returns the error the screen shows, if the engine rejected it.
fn press_action(app: &mut App, action: &RunAction) -> Option<String> {
    match action {
        RunAction::Combat(Action::PlayCard { card, target }) => {
            let state = app.combat().expect("a fight").state();
            let index = state.player.hand.iter().position(|c| c == card);
            press_number(app, index.expect("in hand") + 1);
            if let (Some(target), Input::Target(_)) = (target, app.input()) {
                let state = app.combat().expect("a fight").state();
                let slot = state.enemies.iter().position(|e| e == target);
                press_number(app, slot.expect("an enemy") + 1);
            }
        }
        RunAction::Combat(Action::EndTurn) => app.handle_key(KeyCode::Char('e')),
        RunAction::Combat(Action::UsePotion { slot, target }) => {
            app.handle_key(KeyCode::Char(keys::potion_key(*slot)));
            if let (Some(target), Input::Target(_)) = (target, app.input()) {
                let state = app.combat().expect("a fight").state();
                let at = state.enemies.iter().position(|e| e == target);
                press_number(app, at.expect("an enemy") + 1);
            }
            // A potion that needs no target asks in place: Enter uses it.
            if matches!(app.input(), Input::Potion(_)) {
                app.handle_key(KeyCode::Enter);
            }
        }
        RunAction::Combat(Action::Resolve(_)) => {
            let options = app.decision_options();
            let RunAction::Combat(wanted) = action else {
                unreachable!()
            };
            let index = options
                .iter()
                .position(|o| o == wanted)
                .expect("an offered answer");
            press_number(app, index + 1);
        }
        RunAction::UsePotion(slot) => {
            app.handle_key(KeyCode::Char(keys::potion_key(*slot)));
            if matches!(app.input(), Input::Potion(_)) {
                app.handle_key(KeyCode::Enter);
            }
        }
        // The menu's throw-away row asks in place: its number twice.
        RunAction::DiscardPotion(slot) => {
            app.handle_key(KeyCode::Tab);
            press_number(app, slot + 1);
            press_number(app, slot + 1);
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
                .unwrap_or_else(|| panic!("{action:?} has no menu entry"));
            press_number(app, index + 1);
        }
    }
    // A choice that throws something away asks first: yes.
    if app.confirm_prompt().is_some() {
        app.handle_key(KeyCode::Enter);
    }
    app.error().map(str::to_owned)
}

/// Whether `error` is the engine saying the content is not ported.
fn is_unported(error: &str) -> bool {
    error.contains("not ported")
}

/// `legal` with each `SelectCard` of a deck selection replaced by the first unpicked copy of
/// its card (the one its row takes), each once.
fn first_copies(run: &Run, legal: Vec<RunAction>) -> Vec<RunAction> {
    let RunDecision::CardSelect(select) = run.pending_decision() else {
        return legal;
    };
    let cards = select.cards(&run.state().deck);
    let mut out: Vec<RunAction> = Vec::new();
    for action in legal {
        let action = match action {
            RunAction::SelectCard(n) => RunAction::SelectCard(
                (0..cards.len())
                    .find(|&m| !select.picked.contains(&m) && cards[m] == cards[n])
                    .unwrap_or(n),
            ),
            other => other,
        };
        if !out.contains(&action) {
            out.push(action);
        }
    }
    out
}

/// The policy's candidate actions for the current screen, best first.
fn candidates(app: &App, rng: &mut Rng, plays: usize) -> Vec<RunAction> {
    let run = app.run();
    let mut legal = run.legal_actions();
    // Shuffle so retries after a rejection try something else.
    for i in (1..legal.len()).rev() {
        legal.swap(i, rng.below(i + 1));
    }
    let rank = |a: &RunAction| -> u8 {
        match (run.pending_decision(), a) {
            (_, RunAction::UsePotion(_) | RunAction::DiscardPotion(_)) => 9,
            (RunDecision::Combat(c), RunAction::Combat(act)) => match act {
                Action::Resolve(_) => 0,
                // Attacks first: the bosses with a clock fall before it runs out.
                Action::PlayCard { card, .. } if plays < MAX_PLAYS_PER_TURN => {
                    1 + u8::from(c.state().target_type(*card) != TargetType::AnyEnemy)
                }
                Action::UsePotion { .. } => 4,
                Action::EndTurn => 5,
                Action::PlayCard { .. } => 8,
            },
            (RunDecision::Rewards(_), RunAction::ClaimReward(_) | RunAction::PickCard { .. })
            | (RunDecision::Treasure(_), RunAction::TakeRelic)
            | (RunDecision::RestSite(_), RunAction::Proceed) => 0,
            (RunDecision::Rewards(_), RunAction::SkipReward(_)) => 3,
            (RunDecision::Rewards(_), RunAction::Proceed) => 4,
            (RunDecision::Shop(_), RunAction::Proceed) => 6,
            (RunDecision::Shop(_), _) | (RunDecision::CardSelect(_), RunAction::ConfirmSelect) => 1,
            (RunDecision::CardSelect(_), RunAction::CancelSelect) => 7,
            _ => 2,
        }
    };
    // Skipping one reward has no row of its own (Leave skips what is left); a card reward is
    // skipped on its screen.
    legal.retain(|a| card_reward_of(app, a).is_some() || !matches!(a, RunAction::SkipReward(_)));
    // A deck selection shows identical cards as one row that takes the first unpicked copy
    // (user, 2026-10-30: the copies differ only in the deck's order, which no screen shows), so
    // the copies are one action.
    legal = first_copies(run, legal);
    legal.sort_by_key(rank);
    if let RunDecision::Combat(c) = run.pending_decision()
        && c.status() == Status::PlayerTurn
        && plays >= MAX_PLAYS_PER_TURN
    {
        legal.retain(|a| *a == RunAction::Combat(Action::EndTurn));
    }
    legal
}

/// Plays `run` to its end through the keys. The policy tries its candidates in order until one is
/// accepted; a screen where the engine rejects every legal action for unported content ends the
/// run as `"unported"`.
fn play(mut app: App, policy_seed: u64) -> (App, Outcome) {
    let mut rng = Rng(policy_seed | 1);
    let mut screens = BTreeSet::new();
    let mut unported = BTreeSet::new();
    let mut bosses = BTreeSet::new();
    let mut actions = BTreeSet::new();
    let mut plays = 0;
    let mut last_turn_marker = None;
    for step in 0..MAX_ACTIONS {
        if step % 7 == 0 || app.combat().is_none() {
            smoke(&app, &mut screens);
        }
        if let Room::Combat {
            room_type: RoomType::Boss,
            encounter,
        } = &app.run().state().room
        {
            bosses.insert((app.run().state().act_index, (*encounter).to_owned()));
        }
        if app.is_over() {
            let end = screen(app.run()).0;
            let acts = app.run().state().act_index + 1;
            let killer = killer(&app);
            return (
                app,
                Outcome {
                    end,
                    screens,
                    unported,
                    acts,
                    killer,
                    bosses,
                    actions,
                },
            );
        }
        let marker = app.combat().map(|c| (c.state().round, c.status()));
        if marker != last_turn_marker {
            plays = 0;
            last_turn_marker = marker;
        }
        // Everything the engine lists outside a fight is reachable by keys.
        let candidates = candidates(&app, &mut rng, plays);
        assert!(
            !candidates.is_empty(),
            "no legal action on {:?}",
            app.run().pending_decision()
        );
        if !matches!(app.run().pending_decision(), RunDecision::Combat(_)) {
            for action in &candidates {
                assert!(has_key_path(&app, action), "{action:?} has no key path");
            }
        }
        let mut accepted = false;
        for action in &candidates {
            let before = app.run().clone();
            match press_action(&mut app, action) {
                None => {
                    let name = format!("{action:?}");
                    actions.insert(name.split(['(', ' ', '{']).next().unwrap_or("").to_owned());
                    assert_changed(app.run(), &before, action);
                    accepted = true;
                    break;
                }
                Some(error) if is_unported(&error) => {
                    unported.insert(error);
                    assert_eq!(app.run(), &before, "a rejected action changes nothing");
                }
                Some(error) => panic!("{action:?} was rejected: {error}"),
            }
        }
        if matches!(candidates[0], RunAction::Combat(Action::PlayCard { .. })) {
            plays += 1;
        }
        if !accepted {
            let acts = app.run().state().act_index + 1;
            let killer = killer(&app);
            return (
                app,
                Outcome {
                    end: "unported",
                    screens,
                    unported,
                    acts,
                    killer,
                    bosses,
                    actions,
                },
            );
        }
    }
    panic!(
        "no end within {MAX_ACTIONS} actions: {:?}",
        app.run().pending_decision()
    );
}

/// A run whose player has `SAFE_HP` once Neow is answered (the debug hook needs the map).
fn safe_run(character: Character, seed: &str, ascension: u8) -> App {
    let run = map_run(character, seed, ascension);
    App::with_run_text(seed, ascension, run, Rc::new(Localization::empty()))
}

/// A run that has answered Neow and waits on the map, with `SAFE_HP` and gold.
fn map_run(character: Character, seed: &str, ascension: u8) -> Run {
    let mut run = Run::new(character, seed, ascension, UnlockState::all(), Act1::Random)
        .expect("run")
        .with_events();
    // Neow: the first option that works.
    let options = run.legal_actions();
    let mut answered = false;
    for action in options {
        let mut copy = run.clone();
        if copy.apply(action.clone()).is_ok() {
            run = copy;
            answered = true;
            break;
        }
    }
    assert!(answered, "Neow has an option that works");
    // Settle whatever Neow opened with the first answers, until the map.
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
    state.hp = SAFE_HP;
    state.max_hp = SAFE_HP;
    state.gold = 5_000;
    run
}

fn natural_run(character: Character, seed: &str, ascension: u8) -> Result<App, RunError> {
    App::with_text(character, seed, ascension, Rc::new(Localization::empty()))
}

fn check_outcome(label: &str, outcome: &Outcome) {
    assert!(
        matches!(
            outcome.end,
            "victory" | "game over" | "stopped" | "unported"
        ),
        "{label}: ended on {} ({}, unported {:?})",
        outcome.end,
        outcome.killer,
        outcome.unported
    );
}

/// Plays `seeds` with a safe player until one run is won (the policy's fights are not smart, so
/// some seeds lose to a boss with a clock); returns that run's app and outcome.
fn first_victory(character: Character, seeds: &[&str], ascension: u8) -> (App, Outcome) {
    for (i, seed) in seeds.iter().enumerate() {
        let app = safe_run(character, seed, ascension);
        let (app, outcome) = play(app, 0x1234_5678 + i as u64);
        check_outcome(&format!("{character:?} {seed} A{ascension}"), &outcome);
        if outcome.end == "victory" {
            return (app, outcome);
        }
    }
    panic!("{character:?} A{ascension}: none of {seeds:?} reached the victory screen");
}

fn assert_victory_screen(app: &App, outcome: &Outcome) {
    assert_eq!(outcome.acts, 3, "the victory comes in Act 3");
    let text = render(app, 120, 44);
    assert!(text.contains("VICTORY"), "{text}");
    assert!(text.contains("Spire is conquered"));
    assert!(text.contains("Retry this seed"));
    for kind in [
        "ancient",
        "map",
        "combat",
        "rewards",
        "rest site",
        "shop",
        "event",
    ] {
        assert!(
            outcome.screens.contains(kind),
            "no {kind} screen on the way: {:?}",
            outcome.screens
        );
    }
}

const SEEDS: [&str; 24] = [
    "WG1", "WG2", "WG3", "WG4", "WG5", "WG6", "WG7", "WG8", "WG9", "WG10", "WG11", "WG12", "WG13",
    "WG14", "WG15", "WG16", "WG17", "WG18", "WG19", "WG20", "WG21", "WG22", "WG23", "WG24",
];

/// The run-level actions the winning runs between them have taken.
#[test]
fn winning_runs_use_every_common_kind_of_action() {
    let mut taken = BTreeSet::new();
    // Every character's first win, so a content change that reshuffles one seed's choices doesn't
    // drop a kind from the union.
    for character in Character::ALL {
        let (_, outcome) = first_victory(character, &SEEDS, 0);
        taken.extend(outcome.actions);
    }
    for kind in [
        "AncientOption",
        "EventOption",
        "ChooseNode",
        "ClaimReward",
        "PickCard",
        // No `SkipReward`: this policy skips only a reward it fails to claim, and every reward is
        // claimable now; the key path for skipping is checked with the other legal actions.
        "RestOption",
        "SelectCard",
        "TakeRelic",
        "BuyCard",
        "BuyRelic",
        "BuyPotion",
        "RemoveCard",
        "Proceed",
    ] {
        assert!(taken.contains(kind), "no {kind} action in {taken:?}");
    }
}

#[test]
fn ironclad_wins_a_run_through_the_keys() {
    let (app, outcome) = first_victory(Character::Ironclad, &SEEDS, 0);
    assert_victory_screen(&app, &outcome);
}

#[test]
fn silent_wins_a_run_through_the_keys() {
    let (app, outcome) = first_victory(Character::Silent, &SEEDS, 0);
    assert_victory_screen(&app, &outcome);
}

#[test]
fn defect_wins_a_run_through_the_keys() {
    let (app, outcome) = first_victory(Character::Defect, &SEEDS, 0);
    assert_victory_screen(&app, &outcome);
}

#[test]
fn regent_wins_a_run_through_the_keys() {
    let (app, outcome) = first_victory(Character::Regent, &SEEDS, 0);
    assert_victory_screen(&app, &outcome);
}

#[test]
fn necrobinder_wins_a_run_through_the_keys() {
    let (app, outcome) = first_victory(Character::Necrobinder, &SEEDS, 0);
    assert_victory_screen(&app, &outcome);
}

#[test]
fn ascension_10_has_a_second_act_3_boss() {
    let (app, outcome) = first_victory(Character::Ironclad, &SEEDS, 10);
    assert_victory_screen(&app, &outcome);
    let act3: Vec<_> = outcome.bosses.iter().filter(|(act, _)| *act == 2).collect();
    assert_eq!(act3.len(), 2, "A10 ends Act 3 with two bosses: {act3:?}");
    let act1: Vec<_> = outcome.bosses.iter().filter(|(act, _)| *act == 0).collect();
    assert_eq!(act1.len(), 1, "one boss in Act 1: {act1:?}");
}

#[test]
fn natural_runs_of_every_character_end_without_panics() {
    for (i, character) in [
        Character::Ironclad,
        Character::Silent,
        Character::Defect,
        Character::Regent,
        Character::Necrobinder,
    ]
    .into_iter()
    .enumerate()
    {
        let app = natural_run(character, &format!("NAT{i}"), 5).expect("run");
        let (_, outcome) = play(app, 77 + i as u64);
        check_outcome(&format!("{character:?} natural"), &outcome);
    }
}

fn killer(app: &App) -> String {
    format!("{:?}", app.run().state().room)
}

/// A selection that takes "up to" some cards (Gnarled Hammer's enchants) can be confirmed early:
/// the menu has a Done entry that the engine lists as `ConfirmSelect`.
#[test]
fn an_up_to_card_selection_has_a_done_entry() {
    let mut run = map_run(Character::Ironclad, "WG1", 0);
    run.loadout_relic("GNARLED_HAMMER").expect("ported");
    let mut app = App::with_run_text("WG1", 0, run, Rc::new(Localization::empty()));
    assert!(matches!(
        app.run().pending_decision(),
        RunDecision::CardSelect(_)
    ));
    assert!(render(&app, 120, 44).contains("pick up to 3"));
    assert!(
        app.menu()
            .iter()
            .any(|i| i.action == RunAction::ConfirmSelect)
    );
    assert_eq!(press_action(&mut app, &RunAction::SelectCard(0)), None);
    assert!(render(&app, 120, 44).contains("1/3 picked"));
    assert_eq!(press_action(&mut app, &RunAction::ConfirmSelect), None);
    assert!(matches!(app.run().pending_decision(), RunDecision::Map(_)));
}

/// The game's text for the prompt tests: a relic's title and the shared selection prompts.
fn prompt_text() -> Rc<Localization> {
    Rc::new(Localization::from_json(&[
        ("relics", r#"{"TOASTY_MITTENS.title": "Toasty Mittens"}"#),
        (
            "events",
            r#"{"WELLSPRING.title": "Wellspring",
                "WELLSPRING.pages.BATHE.description": "The water washes you clean."}"#,
        ),
        (
            "card_selection",
            r#"{"TO_EXHAUST": "Choose {Amount:plural:a card|[blue]{}[/blue] cards} to [gold]Exhaust[/gold].",
                "TO_REMOVE": "Choose {Amount:plural:a card|[blue]{}[/blue] cards} to [gold]Remove[/gold]."}"#,
        ),
    ]))
}

/// A fight's card choice says what asks and the game's prompt (Toasty Mittens' exhaust at the
/// start of the first turn), not a guess from the last card played.
#[test]
fn a_fight_choice_names_what_asks() {
    let mut run = map_run(Character::Ironclad, "WG1", 0);
    run.loadout_relic("TOASTY_MITTENS").expect("ported");
    for _ in 0..20 {
        if run.combat().is_some_and(|c| c.status() == Status::Decision) {
            break;
        }
        let next = run
            .legal_actions()
            .into_iter()
            .find(|a| run.clone().apply(a.clone()).is_ok())
            .expect("a way on");
        run.apply(next).expect("legal");
    }
    assert!(
        run.combat().is_some_and(|c| c.status() == Status::Decision),
        "Toasty Mittens asks at the first turn"
    );
    let app = App::with_run_text("WG1", 0, run, prompt_text());
    let screen = render(&app, 120, 44);
    assert!(
        screen.contains("Toasty Mittens · Choose a card to Exhaust (0 of 1)"),
        "{screen}"
    );
}

/// An event's deck selection is titled with the event and shows its prompt.
#[test]
fn an_event_selection_shows_its_prompt() {
    let mut run = map_run(Character::Ironclad, "WG1", 0);
    run.debug_enter_event(EventKind::Wellspring)
        .expect("Wellspring is ported");
    let bathe = run
        .legal_actions()
        .into_iter()
        .nth(1)
        .expect("Wellspring's second option, Bathe");
    run.apply(bathe).expect("legal");
    assert!(matches!(run.pending_decision(), RunDecision::CardSelect(_)));
    let mut app = App::with_run_text("WG1", 0, run, prompt_text());
    let screen = render(&app, 120, 44);
    assert!(screen.contains("Wellspring"), "{screen}");
    assert!(screen.contains("Choose a card to Remove."), "{screen}");
    // The event finished on its Bathe page: shown above the selection and on the map after.
    assert!(screen.contains("The water washes you clean."), "{screen}");
    assert_eq!(press_action(&mut app, &RunAction::SelectCard(0)), None);
    assert!(matches!(app.run().pending_decision(), RunDecision::Map(_)));
    let screen = render(&app, 120, 44);
    assert!(screen.contains("The water washes you clean."), "{screen}");
    // Until the next room.
    let travel = app.run().legal_actions()[0].clone();
    assert_eq!(press_action(&mut app, &travel), None);
    assert!(app.run().finished_event().is_none());
}

/// `run` played on with the first working actions until a fight starts.
fn into_first_fight(mut run: Run) -> Run {
    for _ in 0..20 {
        if run.combat().is_some() {
            return run;
        }
        let next = run
            .legal_actions()
            .into_iter()
            .find(|a| run.clone().apply(a.clone()).is_ok())
            .expect("a way on");
        run.apply(next).expect("legal");
    }
    panic!("no fight within 20 actions");
}

/// The Info overlay explains the orbs with their values now (Cracked Core's Lightning) and
/// Osty's HP.
#[test]
fn info_explains_orbs_and_osty() {
    let text = Rc::new(Localization::from_json(&[(
        "orbs",
        r#"{"LIGHTNING_ORB.title": "Lightning",
            "LIGHTNING_ORB.smartDescription": "[gold]Passive:[/gold] deal [blue]{Passive}[/blue].\n[gold]Evoke:[/gold] deal [blue]{Evoke}[/blue].",
            "EMPTY_SLOT.title": "Orb Slot", "EMPTY_SLOT.description": "Orbs go here."}"#,
    )]));
    let run = into_first_fight(map_run(Character::Defect, "WG1", 0));
    let mut app = App::with_run_text("WG1", 0, run, Rc::clone(&text));
    app.handle_key(KeyCode::Char('i'));
    let screen = render(&app, 120, 60);
    assert!(screen.contains("1. Lightning 3/8"), "{screen}");
    assert!(screen.contains("Passive: deal 3."), "{screen}");
    assert!(screen.contains("Evoke: deal 8."), "{screen}");
    assert!(screen.contains("2. Orb Slot"), "{screen}");

    let run = into_first_fight(map_run(Character::Necrobinder, "WG1", 0));
    let mut app = App::with_run_text("WG1", 0, run, text);
    app.handle_key(KeyCode::Char('i'));
    let screen = render(&app, 120, 60);
    assert!(screen.contains("Osty  HP "), "{screen}");
}

/// Trial's case page wraps the case's story (`TRIAL.trialFormat`), and its verdict finishes
/// on `TRIAL.trialResult` around the verdict's page.
#[test]
fn trial_pages_show_their_story_and_result() {
    let cases = ["MERCHANT", "NOBLE", "NONDESCRIPT"];
    let mut events = String::from(
        r#"{"TRIAL.title": "Trial", "TRIAL.trialFormat": "The trial begins. {TrialStory} Decide!",
            "TRIAL.trialResult": "IT IS DECIDED. {TrialResult}""#,
    );
    for case in cases {
        write!(
            events,
            r#", "TRIAL.pages.{case}.description": "The {case} story.",
                "TRIAL.pages.{case}_GUILTY.description": "The {case} is guilty.",
                "TRIAL.pages.{case}_INNOCENT.description": "The {case} is innocent.""#
        )
        .expect("writing to a String");
    }
    events.push('}');
    let text = Rc::new(Localization::from_json(&[("events", &events)]));
    let mut run = map_run(Character::Ironclad, "WG1", 0);
    run.debug_enter_event(EventKind::Trial)
        .expect("Trial is ported");
    run.apply(RunAction::EventOption(0)).expect("ACCEPT");
    let app = App::with_run_text("WG1", 0, run.clone(), Rc::clone(&text));
    let screen = render(&app, 120, 44);
    let case = cases
        .into_iter()
        .find(|c| screen.contains(&format!("The trial begins. The {c} story. Decide!")))
        .unwrap_or_else(|| panic!("a case story: {screen}"));
    run.apply(RunAction::EventOption(0)).expect("GUILTY");
    let app = App::with_run_text("WG1", 0, run, text);
    let screen = render(&app, 120, 44);
    assert!(
        screen.contains(&format!("IT IS DECIDED. The {case} is guilty.")),
        "{screen}"
    );
}

/// With the game's text (`$STS2_LOCALIZATION`): every event's first page and options, with the
/// placeholders left unfilled listed.
#[test]
#[ignore = "needs the game's localization"]
fn unfilled_event_placeholders() {
    let text = Rc::new(sts2_tui::text::discover());
    let mut left = Vec::new();
    for kind in EventKind::ALL {
        let mut run = map_run(Character::Ironclad, "WG1", 0);
        if run.debug_enter_event(kind).is_err() {
            continue;
        }
        let app = App::with_run_text("WG1", 0, run, Rc::clone(&text));
        let screen = render(&app, 200, 80);
        for line in screen.lines().filter(|l| l.contains('‹')) {
            left.push(format!("{}: {}", kind.id(), line.trim()));
        }
    }
    println!("{}", left.join("\n"));
}
