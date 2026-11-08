use std::rc::Rc;

use sts2_core::Character;
use sts2_core::run::{Act1, UnlockState};

use super::*;
use crate::model::Input;
use crate::ui::{Effect, Ui};

fn run_in(fight: bool) -> (Run, Describer) {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S2",
        0,
        UnlockState::all(),
        Act1::Random,
    )
    .expect("run")
    .with_events();
    for _ in 0..60 {
        if !fight || run.combat().is_some() {
            break;
        }
        let action = run.legal_actions().remove(0);
        run.apply(action).expect("legal");
    }
    assert!(
        !fight || run.combat().is_some(),
        "a fight within 60 actions"
    );
    (run, Describer::new(Rc::new(text::Localization::empty())))
}

fn ctx<'a>(run: &'a Run, describer: &'a Describer, visits: &'a [Visit]) -> Context<'a> {
    Context {
        run,
        describer,
        visits,
        tossing: None,
    }
}

fn lines(screen: &Screen) -> Vec<String> {
    match &screen.sheet {
        Some(Sheet::Detail(d)) => d.lines.iter().map(Rich::text).collect(),
        other => panic!("a detail sheet, got {other:?}"),
    }
}

#[test]
fn the_deck_groups_identical_cards() {
    let (run, describer) = run_in(false);
    let screen = screen(Kind::Deck, &ctx(&run, &describer, &[])).expect("deck");
    let lines = lines(&screen);
    assert!(lines.iter().any(|l| l.starts_with("4x ")), "{lines:?}");
    let title = screen.title.expect("title").text();
    assert_eq!(title, format!("Deck ({} cards)", run.state().deck.len()));
}

#[test]
fn a_pile_is_sorted_and_the_draw_pile_hides_its_order() {
    let (run, describer) = run_in(true);
    let draw = screen(Kind::DrawPile, &ctx(&run, &describer, &[])).expect("draw");
    assert!(
        draw.title
            .expect("t")
            .text()
            .ends_with(", the order is hidden")
    );
    let exhaust = screen(Kind::ExhaustPile, &ctx(&run, &describer, &[])).expect("exhaust");
    assert_eq!(lines(&exhaust), ["(empty)"]);
    let (outside, describer) = run_in(false);
    assert!(screen(Kind::DrawPile, &ctx(&outside, &describer, &[])).is_none());
    assert!(screen(Kind::Log, &ctx(&outside, &describer, &[])).is_none());
}

#[test]
fn the_history_shows_hp_and_gold_per_floor() {
    let (run, describer) = run_in(false);
    let visits = [
        Visit {
            act: 1,
            floor: 1,
            what: "Fight: Jaw Worm".to_owned(),
            hp_in: 70,
            hp_out: 60,
            max_hp: 70,
            gold_out: 12,
        },
        Visit {
            act: 1,
            floor: 2,
            what: "Shop".to_owned(),
            hp_in: 60,
            hp_out: 60,
            max_hp: 70,
            gold_out: 12,
        },
    ];
    let screen = screen(Kind::History, &ctx(&run, &describer, &visits)).expect("history");
    assert_eq!(
        lines(&screen),
        [
            "Act 1",
            "Floor 1 · ♥ 70 → 60/70 · ◉ 12",
            "   Fight: Jaw Worm",
            "Floor 2 · ♥ 60/70 · ◉ 12",
            "   Shop",
        ]
    );
    assert_eq!(screen.title.expect("t").text(), "History (2 rooms)");
}

#[test]
fn the_menu_lists_the_fights_piles_only_in_a_fight() {
    let keys = |fight| -> Vec<char> { menu_entries(fight).iter().map(|e| e.key).collect() };
    assert_eq!(keys(false), ['d', 'v', 'i', 'm', 'h', '?', 'o', 'q']);
    assert_eq!(
        keys(true),
        ['d', 'v', 'i', 'm', 'l', 'a', 's', 'x', 'h', '?', 'o', 'q']
    );
}

#[test]
fn the_menu_has_a_throw_away_row_per_potion_under_the_potions_entry() {
    let (run, describer) = run_in(true);
    let rows = menu_rows(&ctx(&run, &describer, &[]));
    let held = run.potions().iter().flatten().count();
    let tosses = rows
        .iter()
        .filter(|r| matches!(r.entry, Entry::Toss(_)))
        .count();
    assert_eq!(tosses, held);
    let ids: Vec<u32> = rows.iter().map(|r| r.button.id.0).collect();
    assert_eq!(
        ids,
        (1..=u32::try_from(rows.len()).expect("small")).collect::<Vec<_>>()
    );
    if held > 0 {
        let at = rows
            .iter()
            .position(|r| r.entry == Entry::Sheet(Kind::Info))
            .expect("info");
        assert!(matches!(rows[at + 1].entry, Entry::Toss(0..=2)));
    }
}

#[test]
fn the_full_map_is_the_screens_grid() {
    let (run, describer) = run_in(false);
    let screen = screen(Kind::Map, &ctx(&run, &describer, &[])).expect("map");
    assert!(matches!(screen.blocks.first(), Some(Block::Map(g)) if !g.cells.is_empty()));
    assert!(lines(&screen)[0].starts_with("M monster"));
}

#[test]
fn the_ui_opens_toggles_and_closes_a_sheet() {
    let (run, describer) = run_in(false);
    let mut ui = Ui::default();
    assert_eq!(ui.toggle_sheet(&run, Kind::Deck), []);
    assert_eq!(ui.sheet(), Some(Kind::Deck));
    ui.scroll_sheet(3);
    assert_eq!(ui.sheet_scroll(), 3);
    ui.scroll_sheet(-9);
    assert_eq!(ui.sheet_scroll(), 0);
    // A list key from the menu toggles that list.
    ui.toggle_sheet(&run, Kind::Menu);
    let _ = ui.sheet_input(&run, &describer, &Input::Key(Key::Char('v')));
    assert_eq!(ui.sheet(), Some(Kind::Relics));
    assert_eq!(ui.sheet_input(&run, &describer, &Input::Back), []);
    assert_eq!(ui.sheet(), None);
    // The piles exist only in a fight.
    assert!(matches!(
        ui.open_sheet(&run, Kind::DrawPile).as_slice(),
        [Effect::Error(_)]
    ));
    assert_eq!(ui.sheet(), None);
}

#[test]
fn a_menu_tap_opens_a_list_or_asks_the_client() {
    let (run, describer) = run_in(false);
    let mut ui = Ui::default();
    ui.open_sheet(&run, Kind::Menu);
    let quit = menu_rows(&ctx(&run, &describer, &[]))
        .into_iter()
        .find(|r| r.entry == Entry::Quit)
        .expect("quit");
    assert_eq!(
        ui.sheet_input(&run, &describer, &Input::Tap(quit.button.id)),
        [Effect::Menu(Entry::Quit)]
    );
    assert_eq!(ui.sheet(), Some(Kind::Menu));
    let _ = ui.sheet_input(&run, &describer, &Input::Tap(ButtonId(1)));
    assert_eq!(ui.sheet(), Some(Kind::Deck));
}

#[test]
fn the_logs_scroll_counts_from_its_newest_line() {
    let (run, describer) = run_in(true);
    let mut ui = Ui::default();
    ui.open_sheet(&run, Kind::Log);
    ui.scroll_sheet(-4);
    assert_eq!(ui.sheet_scroll(), 4);
    let _ = ui.sheet_input(&run, &describer, &Input::Key(Key::Down));
    assert_eq!(ui.sheet_scroll(), 3);
}
