//! Enchantments on the cards the TUI shows, and what the character select says about the
//! ascension: the starting HP and gold from the engine, and the modifiers.

use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::KeyCode;
use sts2_core::Character;
use sts2_core::content::enchantments::{Enchantment, EnchantmentKind};
use sts2_core::run::ascension::MAX_ASCENSION;
use sts2_core::run::{Act1, Run, RunAction, RunDecision, UnlockState};
use sts2_tui::app::App;
use sts2_tui::shell::Shell;
use sts2_tui::text::Localization;
use sts2_tui::ui;

fn text_of(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(Cell::symbol)
        .collect()
}

fn render_app(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(110, 42)).expect("test backend");
    terminal.draw(|f| ui::draw(f, app)).expect("draw");
    text_of(&terminal)
}

fn render_shell(shell: &Shell) -> String {
    let mut terminal = Terminal::new(TestBackend::new(110, 42)).expect("test backend");
    terminal.draw(|f| ui::draw_shell(f, shell)).expect("draw");
    text_of(&terminal)
}

/// A run whose first deck card is enchanted with Sharp 2.
fn enchanted_app(text: Localization) -> App {
    let run = Run::new(
        Character::Ironclad,
        "ENCHANT",
        0,
        UnlockState::all(),
        Act1::Random,
    )
    .expect("run");
    // Past Neow: the first of her options that leaves the run on the map.
    let neow = run.clone();
    let mut run = run
        .legal_actions()
        .into_iter()
        .filter(|a| matches!(a, RunAction::AncientOption(_)))
        .find_map(|a| {
            let mut after = neow.clone();
            after.apply(a).ok()?;
            matches!(after.pending_decision(), RunDecision::Map(_)).then_some(after)
        })
        .expect("a Neow option that ends on the map");
    run.loadout_state().expect("on the map").deck[0].enchantment =
        Some(Enchantment::new(EnchantmentKind::Sharp, 2));
    App::with_run_text("ENCHANT", 0, run, Rc::new(text))
}

#[test]
fn an_enchanted_card_shows_its_tag_in_the_deck_overlay() {
    let mut app = enchanted_app(Localization::empty());
    app.handle_key(KeyCode::Char('d'));
    let screen = render_app(&app);
    assert!(screen.contains("✦Sharp 2"), "{screen}");
    assert!(screen.contains("✦ Sharp"), "the detail line names it");
}

#[test]
fn the_deck_overlay_shows_the_enchantment_text() {
    let text = Localization::from_json(&[
        (
            "cards",
            r#"{"STRIKE_IRONCLAD.description": "Deal {Damage} damage."}"#,
        ),
        (
            "enchantments",
            r#"{"SHARP.title": "Sharp", "SHARP.description": "Increases damage on this card by [blue]{Amount}[/blue]."}"#,
        ),
    ]);
    let mut app = enchanted_app(text);
    app.handle_key(KeyCode::Char('d'));
    let screen = render_app(&app);
    assert!(screen.contains("Increases damage on this card"), "{screen}");
}

#[test]
fn a_plain_card_has_no_tag() {
    let mut app = enchanted_app(Localization::empty());
    app.handle_key(KeyCode::Char('d'));
    // Only the one enchanted card carries it.
    assert_eq!(render_app(&app).matches("✦Sharp").count(), 1);
}

fn select_text(character_key: char, level: u8) -> String {
    let mut shell = Shell::select(Rc::new(Localization::empty()));
    shell.handle_key(KeyCode::Char(character_key));
    for _ in 0..level {
        shell.handle_key(KeyCode::Up);
    }
    render_shell(&shell)
}

/// The `♥ hp/max` shown for the chosen character.
fn start_hp(screen: &str, character: Character) -> i32 {
    assert!(screen.contains(&character.name().to_uppercase()));
    let marker = "♥ ";
    let rest = &screen[screen.find(marker).expect("the start line") + marker.len()..];
    rest.split('/')
        .next()
        .expect("hp")
        .parse()
        .expect("a number")
}

#[test]
fn the_select_screen_shows_lower_hp_at_weary_traveler_for_every_character() {
    for (i, character) in sts2_tui::shell::SELECT_ORDER.into_iter().enumerate() {
        let key = char::from_digit(u32::try_from(i + 1).expect("small"), 10).expect("digit");
        let a0 = start_hp(&select_text(key, 0), character);
        let a1 = start_hp(&select_text(key, 1), character);
        let a2 = start_hp(&select_text(key, 2), character);
        let a10 = start_hp(&select_text(key, MAX_ASCENSION), character);
        assert_eq!(a0, character.starting_hp());
        assert_eq!(a1, a0, "{}: A1 changes nothing", character.name());
        assert!(a2 < a0, "{}: {a2} should be below {a0}", character.name());
        assert_eq!(a10, a2);
    }
}

#[test]
fn the_select_screen_shows_the_level_s_modifier() {
    let text = Localization::from_json(&[(
        "ascension",
        r#"{"LEVEL_01.title": "Swarming Elites", "LEVEL_01.description": "Elites spawn more often.",
            "LEVEL_02.title": "Weary Traveler", "LEVEL_02.description": "Ancients only heal 80% of your missing HP.",
            "LEVEL_03.title": "Poverty", "LEVEL_03.description": "Less gold."}"#,
    )]);
    let mut shell = Shell::select(Rc::new(text));
    for _ in 0..2 {
        shell.handle_key(KeyCode::Up);
    }
    let screen = render_shell(&shell);
    assert!(screen.contains("Ascension 2"));
    assert!(screen.contains("Weary Traveler: Ancients only heal 80%"));
    assert!(
        !screen.contains("Swarming Elites"),
        "one line: the level's own"
    );
    assert!(!screen.contains("Poverty"), "level 3 is not on yet");
}
