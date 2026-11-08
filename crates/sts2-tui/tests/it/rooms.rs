//! Snapshots of the rooms drawn from `sts2-view`'s `Screen` (Neow, events, rest sites, treasure,
//! card selections and bundles) at the four sizes of the design: a phone over SSH (45×30), a
//! classic 80×24, a laptop (120×35) and a large terminal (160×45). The fixed seed and the made-up
//! text table are `snapshots`'.
//!
//! A changed screen fails with the new render written beside the old one; run with
//! `UPDATE_SNAPSHOTS=1` to accept the new ones.

use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use sts2_core::Character;
use sts2_core::run::{
    Act1, MapCoord, MapPointType, RestOptionKind, Run, RunAction, RunDecision, UnlockState,
};
use sts2_tui::app::App;
use sts2_tui::hit::UiAction;
use sts2_tui::ui;

use super::snapshots::{SEED, dir, map_run, text, text_of, win};

const SIZES: [(u16, u16); 4] = [(45, 30), (80, 24), (120, 35), (160, 45)];

fn render(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
    terminal.draw(|f| ui::draw(f, app)).expect("draw");
    text_of(terminal.backend().buffer(), width)
}

/// Compares `app`'s screen at every size with its snapshot.
fn check(name: &str, app: &App) {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let mut failed = Vec::new();
    for (width, height) in SIZES {
        let file = dir().join(format!("{name}-{width}x{height}.txt"));
        let drawn = render(app, width, height);
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

/// A new run, on Neow.
fn neow() -> App {
    let run = Run::new(
        Character::Ironclad,
        SEED,
        0,
        UnlockState::all(),
        Act1::Random,
    )
    .expect("run")
    .with_events();
    let app = App::with_run_text(SEED, 0, run, text());
    assert!(matches!(
        app.run().pending_decision(),
        RunDecision::Ancient(_)
    ));
    app
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

/// Plays on, fights won with the keys with HP to spare, rewards left behind, taking the way
/// to `kind` on the map, until `reached` says the screen is the one.
fn reach(kind: MapPointType, reached: impl Fn(&App) -> bool) -> App {
    let mut app = App::with_run_text(SEED, 0, map_run(999), text());
    for _ in 0..200 {
        if reached(&app) {
            return app;
        }
        match app.run().pending_decision() {
            RunDecision::Map(nodes) => {
                let next = nodes
                    .iter()
                    .copied()
                    .min_by_key(|&c| distance_to(app.run(), c, kind))
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
    panic!("the room was not reached");
}

fn event() -> App {
    reach(MapPointType::Unknown, |app| {
        matches!(app.run().pending_decision(), RunDecision::Event(_))
    })
}

#[test]
fn neow_and_events_match_their_snapshots() {
    check("neow", &neow());
    check("event", &event());
}

fn rest() -> App {
    reach(MapPointType::RestSite, |app| {
        matches!(app.run().pending_decision(), RunDecision::RestSite(_))
    })
}

/// The first rest site with Smith pressed: the deck selection with its upgrade preview.
fn smith() -> App {
    let mut app = rest();
    let RunDecision::RestSite(options) = app.run().pending_decision() else {
        panic!("a rest site");
    };
    let smith = options
        .iter()
        .position(|o| o.kind == RestOptionKind::Smith)
        .expect("Smith");
    let n = app
        .menu()
        .iter()
        .filter(|i| i.enabled)
        .position(|i| i.action == RunAction::RestOption(smith))
        .expect("an entry");
    app.perform(UiAction::Pick(n + 1));
    assert!(matches!(
        app.run().pending_decision(),
        RunDecision::CardSelect(_)
    ));
    app
}

fn treasure() -> App {
    reach(MapPointType::Treasure, |app| {
        matches!(app.run().pending_decision(), RunDecision::Treasure(_))
    })
}

/// Neow's Scroll Boxes: the choose-a-bundle screen.
fn bundles() -> App {
    let mut app = neow();
    let RunDecision::Ancient(room) = app.run().pending_decision() else {
        panic!("Neow");
    };
    let at = room
        .options
        .iter()
        .position(|o| o.name() == "SCROLL_BOXES")
        .expect("Scroll Boxes on offer");
    app.perform(UiAction::Pick(at + 1));
    assert!(matches!(
        app.run().pending_decision(),
        RunDecision::Bundles(_)
    ));
    app
}

#[test]
fn rest_treasure_selection_and_bundle_screens_match_their_snapshots() {
    check("rest", &rest());
    check("smith", &smith());
    check("treasure", &treasure());
    check("bundles", &bundles());
}
