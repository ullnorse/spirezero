//! The session: the autosave and Continue (random runs saved and played back to the same
//! digest), the settings file, the crash note, the boss screen, and the help listing every key
//! the input handlers take.

use std::path::PathBuf;
use std::rc::Rc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use sts2_core::run::{MapCoord, RunAction};
use sts2_core::{Character, Status};
use sts2_tui::app::{App, Input};
use sts2_tui::hit::{Target, UiAction};
use sts2_tui::keys::{self, Command, Keys};
use sts2_tui::session::{Dirs, Save, Session};
use sts2_tui::settings::{BossView, Settings, Theme};
use sts2_tui::shell::{Mode, Panel, Shell};
use sts2_tui::text::Localization;
use sts2_tui::ui;

/// A fresh directory for one test's save and settings.
fn scratch(name: &str) -> Dirs {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("session")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    Dirs::at(&dir)
}

fn text() -> Rc<Localization> {
    Rc::new(Localization::empty())
}

/// The game as launched from `dirs`, like `main` does.
fn launch(dirs: &Dirs) -> Shell {
    Shell::launch(
        text(),
        Settings::load(&dirs.settings()),
        Some(Session::new(dirs.clone())),
        None,
    )
}

fn app(shell: &Shell) -> &App {
    match shell.mode() {
        Mode::Play(app) => app,
        Mode::Select(_) => panic!("not in a run"),
    }
}

fn press(shell: &mut Shell, keys: &str) {
    for c in keys.chars() {
        shell.handle_key(KeyCode::Char(c));
    }
}

fn screen(shell: &mut Shell, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
    ui::render(&mut terminal, shell).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(Cell::symbol)
        .collect()
}

/// Draws the shell, then sends `kind` at the middle of where `target` was drawn, as a
/// terminal would report it.
fn mouse(shell: &mut Shell, kind: MouseEventKind, target: Target) {
    let _ = screen(shell, 150, 46);
    let rect = shell
        .hits()
        .rect_of(target)
        .unwrap_or_else(|| panic!("{target:?} is not on the screen"));
    let pos = Position::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
    shell.handle_mouse(MouseEvent {
        kind,
        column: pos.x,
        row: pos.y,
        modifiers: KeyModifiers::NONE,
    });
}

fn hover(shell: &mut Shell, target: Target) {
    mouse(shell, MouseEventKind::Moved, target);
}

/// Moves over `target`, presses and releases there.
fn click(shell: &mut Shell, target: Target) {
    hover(shell, target);
    mouse(shell, MouseEventKind::Down(MouseButton::Left), target);
    mouse(shell, MouseEventKind::Up(MouseButton::Left), target);
}

/// A run started from the select screen and played a few actions, then quit: the save it
/// leaves holds those actions.
fn play_and_quit(dirs: &Dirs) -> Vec<u32> {
    let mut shell = launch(dirs);
    shell.handle_key(KeyCode::Enter);
    let mut rng = 7;
    for _ in 0..6 {
        random_input(&mut shell, &mut rng);
    }
    let actions = app(&shell).actions().to_vec();
    assert!(actions.len() > 1, "some actions played");
    press(&mut shell, "q");
    assert!(shell.should_quit());
    assert_eq!(
        Session::new(dirs.clone()).load().expect("save").actions,
        actions
    );
    actions
}

/// A xorshift step.
const fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}

/// One random input of a player: a card, an option, a target, a potion, the end of the turn,
/// an answer to a question.
fn random_input(shell: &mut Shell, rng: &mut u64) {
    let app = app(shell);
    if app.confirm_prompt().is_some() {
        shell.handle_key(if next(rng).is_multiple_of(2) {
            KeyCode::Enter
        } else {
            KeyCode::Char('z')
        });
        return;
    }
    if matches!(app.input(), Input::Target(_)) {
        let n = next(rng) % 3 + 1;
        press(shell, &n.to_string());
        if matches!(app_input(shell), Input::Target(_)) {
            shell.handle_key(KeyCode::Esc);
        }
        return;
    }
    if matches!(app.input(), Input::Potion(_)) {
        shell.handle_key(if next(rng).is_multiple_of(2) {
            KeyCode::Enter
        } else {
            KeyCode::Esc
        });
        return;
    }
    let count = match app.combat() {
        Some(c) if c.status() == Status::PlayerTurn => c.state().player.hand.len() + 1,
        Some(c) => app
            .decision_options()
            .len()
            .max(c.state().player.hand.len()),
        None => app.menu().iter().filter(|i| i.enabled).count(),
    };
    let roll = next(rng);
    let in_turn = app
        .combat()
        .is_some_and(|c| c.status() == Status::PlayerTurn);
    match roll % 40 {
        0 => press(shell, "z"),
        1 => press(shell, "c"),
        _ if count == 0 => {}
        // Play cards until one cannot be played, then end the turn.
        _ if in_turn => {
            let pick = usize::try_from(next(rng) % (count as u64 - 1).max(1)).unwrap_or(0) + 1;
            press(shell, &pick.to_string());
            if matches!(app_input(shell), Input::Target(_)) {
                let n = next(rng) % 2 + 1;
                press(shell, &n.to_string());
            }
            if self::app(shell).error().is_some() || matches!(app_input(shell), Input::Target(_)) {
                shell.handle_key(KeyCode::Esc);
                press(shell, "e");
            }
        }
        _ => {
            let pick = usize::try_from(next(rng) % count as u64).unwrap_or(0) + 1;
            // Past ten the list is paged: the digit picks on the page the focus is on.
            let paging = app.paging();
            if let Some(want) = paging.page_of(pick) {
                let have = app.page();
                let turn = if want > have {
                    KeyCode::PageDown
                } else {
                    KeyCode::PageUp
                };
                for _ in 0..want.abs_diff(have) {
                    shell.handle_key(turn);
                }
            }
            press(shell, &paging.key(pick));
        }
    }
}

fn app_input(shell: &Shell) -> Input {
    app(shell).input().clone()
}

/// The save on disk, played back: the same run, digest and rooms as the app shown.
fn assert_save_restores(dirs: &Dirs, shell: &Shell) {
    let live = app(shell);
    let save = Session::new(dirs.clone()).load().expect("a save");
    assert_eq!(save.actions, live.actions());
    assert_eq!(Save::parse(&save.to_text()).as_ref(), Ok(&save));
    let old_name = save
        .to_text()
        .replacen("spirezero-save", "spirebot-save", 1);
    assert_eq!(
        Save::parse(&old_name).as_ref(),
        Ok(&save),
        "saves from before the rename load"
    );
    let restored = save.restore(text()).expect("restores");
    assert!(restored.exact, "digest differs");
    assert_eq!(restored.app.digest(), live.digest());
    assert_eq!(restored.app.run(), live.run());
    assert_eq!(restored.app.visits(), live.visits());
}

#[test]
fn random_runs_are_saved_after_every_action_and_continue_where_they_were() {
    let characters = [
        Character::Necrobinder,
        Character::Ironclad,
        Character::Silent,
        Character::Defect,
        Character::Regent,
    ];
    let mut ended = 0;
    for n in 0..20_u64 {
        let dirs = scratch(&format!("random{n}"));
        let character = characters[usize::try_from(n).unwrap_or(0) % characters.len()];
        let start = App::with_text(
            character,
            &format!("SAVE{n}"),
            u8::try_from(n % 3 * 5).unwrap_or(0),
            text(),
        )
        .expect("run");
        let mut shell = Shell::launch(
            text(),
            Settings::default(),
            Some(Session::new(dirs.clone())),
            Some(start),
        );
        let mut rng = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        // Random play dies soon: the runs stop at different lengths, so some are still going.
        for step in 0..30 + 25 * (n % 6) {
            if app(&shell).is_over() {
                break;
            }
            random_input(&mut shell, &mut rng);
            if step % 40 == 39 && !app(&shell).is_over() {
                assert_save_restores(&dirs, &shell);
            }
        }
        if app(&shell).is_over() {
            ended += 1;
            assert!(!dirs.save().exists(), "an ended run leaves no save");
            continue;
        }
        assert_save_restores(&dirs, &shell);
        // Quit (no question asked), launch again: Continue is offered and Enter lands on the
        // same state.
        let before = app(&shell).digest();
        // A question left open takes the next key as its answer: close it first.
        if app(&shell).confirm_prompt().is_some() {
            press(&mut shell, "n");
        }
        press(&mut shell, "q");
        assert!(shell.should_quit());
        let mut again = launch(&dirs);
        let Mode::Select(select) = again.mode() else {
            panic!("the select screen first");
        };
        assert!(select.continue_focused());
        let save = select.saved().expect("the save");
        let summary = save.summary();
        assert!(summary.starts_with(character.name()), "{summary}");
        let place = format!("Act {} · Floor {}", save.act, save.floor);
        let menu = screen(&mut again, 120, 40);
        assert!(
            menu.contains(character.name()) && menu.contains(&place),
            "{menu}"
        );
        again.handle_key(KeyCode::Enter);
        assert_eq!(app(&again).digest(), before);
        assert_eq!(app(&again).error(), None);
    }
    assert!(
        (1..=14).contains(&ended),
        "{ended} of 20 runs ended: both kinds are wanted"
    );
}

/// Long runs: the greedy policy plays through the shell (saved after every action) until the
/// run ends or reaches Act 2; the save of every hundredth state and of the last plays back to
/// the same digest.
#[test]
fn greedy_runs_save_and_continue_over_whole_acts() {
    use sts2_core::run::RunDecision;
    use sts2_diff::greedy::Greedy;
    let mut longest = 0;
    for (n, character) in [
        Character::Necrobinder,
        Character::Ironclad,
        Character::Silent,
    ]
    .into_iter()
    .enumerate()
    {
        let dirs = scratch(&format!("greedy{n}"));
        let start =
            App::with_text(character, &format!("RL2S{}", 2 + 17 * n), 0, text()).expect("run");
        let mut shell = Shell::launch(
            text(),
            Settings::default(),
            Some(Session::new(dirs.clone())),
            Some(start),
        );
        let mut planner = Greedy::default();
        for step in 0..2500 {
            let run = app(&shell).run();
            if app(&shell).is_over() || run.state().act_index == 2 {
                break;
            }
            let legal = run.legal_actions();
            if legal.is_empty() || matches!(run.pending_decision(), RunDecision::Stopped(_)) {
                break;
            }
            let action = match run.combat().and_then(Greedy::answer) {
                Some(answer) if legal.contains(&RunAction::Combat(answer.clone())) => {
                    RunAction::Combat(answer)
                }
                _ => planner.choose(run, &legal),
            };
            shell.apply(action);
            assert_eq!(app(&shell).error(), None, "step {step}");
            if step % 100 == 99 && !app(&shell).is_over() {
                assert_save_restores(&dirs, &shell);
            }
        }
        longest = longest.max(app(&shell).actions().len());
        if !app(&shell).is_over() {
            assert_save_restores(&dirs, &shell);
        }
    }
    assert!(longest > 300, "the longest run took {longest} actions");
}

#[test]
fn a_new_run_replaces_the_save_and_remembers_the_choices() {
    let dirs = scratch("new-run");
    let mut shell = launch(&dirs);
    assert!(!dirs.save().exists());
    press(&mut shell, "3++");
    shell.handle_key(KeyCode::Enter);
    assert_eq!(app(&shell).run().state().character, Character::Regent);
    let save = Session::new(dirs.clone()).load().expect("saved at once");
    assert_eq!(save.actions, Vec::<u32>::new());
    press(&mut shell, "1");
    assert_eq!(
        Session::new(dirs.clone())
            .load()
            .expect("save")
            .actions
            .len(),
        1
    );
    // The next launch starts on Regent A2 with the run offered; `n` sets up a new one instead.
    let mut again = launch(&dirs);
    press(&mut again, "n");
    let Mode::Select(select) = again.mode() else {
        panic!("select");
    };
    assert!(!select.continue_focused());
    assert_eq!(select.character(), Character::Regent);
    assert_eq!(select.ascension(), 2);
    again.handle_key(KeyCode::Enter);
    assert_eq!(
        app(&again).actions().len(),
        0,
        "a new run, not the saved one"
    );
    assert_eq!(Session::new(dirs).load().expect("save").actions.len(), 0);
}

#[test]
fn after_a_crash_the_run_continues_from_before_the_last_action_too() {
    let dirs = scratch("crash");
    let mut shell = launch(&dirs);
    shell.handle_key(KeyCode::Enter);
    press(&mut shell, "1");
    let after_neow = app(&shell).digest();
    // One more action, whatever the screen offers.
    for n in 1..=9 {
        if app(&shell).actions().len() == 2 {
            break;
        }
        press(&mut shell, &n.to_string());
    }
    assert_eq!(app(&shell).actions().len(), 2);
    let note = "sts2-tui crashed: test";
    std::fs::write(dirs.crash(), note).expect("note");
    let mut again = launch(&dirs);
    let Mode::Select(select) = again.mode() else {
        panic!("select");
    };
    assert!(select.crash_note().is_some());
    assert!(select.can_continue_earlier());
    assert!(!dirs.crash().exists(), "the note is shown once");
    assert!(dirs.old_crash().exists());
    let drawn = screen(&mut again, 160, 40);
    assert!(drawn.contains("crashed"), "the crash is mentioned");
    press(&mut again, "b");
    assert_eq!(app(&again).actions().len(), 1);
    assert_eq!(app(&again).digest(), after_neow);
}

#[test]
fn a_save_that_no_longer_applies_says_so_and_offers_a_new_run() {
    let dirs = scratch("stale");
    let mut save = {
        let mut shell = launch(&dirs);
        shell.handle_key(KeyCode::Enter);
        Session::new(dirs.clone()).load().expect("save")
    };
    save.actions = vec![9999];
    std::fs::write(dirs.save(), save.to_text()).expect("write");
    let mut shell = launch(&dirs);
    shell.handle_key(KeyCode::Enter);
    let Mode::Select(select) = shell.mode() else {
        panic!("still on the select screen");
    };
    assert!(
        select
            .error()
            .is_some_and(|e| e.contains("cannot be continued"))
    );
    shell.handle_key(KeyCode::Enter);
    assert!(
        matches!(shell.mode(), Mode::Play(_)),
        "Enter now starts a new run"
    );
    // A file that is not a save at all is no save.
    std::fs::write(dirs.save(), "garbage").expect("write");
    assert!(Session::new(dirs).load().is_none());
}

#[test]
fn settings_changed_in_the_game_are_saved_and_read_back() {
    let dirs = scratch("settings");
    let mut shell = launch(&dirs);
    press(&mut shell, "o");
    assert_eq!(shell.panel(), Some(Panel::Settings(0)));
    let drawn = screen(&mut shell, 120, 40);
    assert!(drawn.contains("Discreet mode"));
    // The first row is discreet mode.
    shell.handle_key(KeyCode::Right);
    assert!(shell.settings().discreet);
    // Down to the theme, one step on.
    press(&mut shell, "j");
    shell.handle_key(KeyCode::Enter);
    assert_eq!(shell.settings().theme, Theme::Light);
    // A click on the last row (the boss view) changes it too.
    shell.perform(UiAction::Setting(5));
    assert_eq!(shell.settings().boss_view, BossView::Top);
    shell.handle_key(KeyCode::Esc);
    assert_eq!(shell.panel(), None);
    let saved = Settings::load(&dirs.settings());
    assert_eq!(&saved, shell.settings());
    assert!(saved.discreet);
    // Discreet: greys only on screen.
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("backend");
    ui::render(&mut terminal, &mut shell).expect("draw");
    for cell in terminal.backend().buffer().content() {
        for colour in [cell.fg, cell.bg] {
            assert!(
                matches!(
                    colour,
                    ratatui::style::Color::Reset
                        | ratatui::style::Color::White
                        | ratatui::style::Color::Gray
                        | ratatui::style::Color::DarkGray
                ),
                "{colour:?}"
            );
        }
    }
}

#[test]
fn the_boss_key_hides_everything_and_any_key_comes_back() {
    let dirs = scratch("boss");
    let mut shell = launch(&dirs);
    shell.handle_key(KeyCode::Enter);
    let before = app(&shell).digest();
    for (key, view, marker) in [
        (KeyCode::Char('`'), BossView::Build, "Compiling"),
        (KeyCode::F(12), BossView::Top, "PID USER"),
    ] {
        if shell.settings().boss_view != view {
            press(&mut shell, "o");
            shell.perform(UiAction::Setting(5));
            shell.handle_key(KeyCode::Esc);
        }
        shell.handle_key(key);
        assert!(shell.boss());
        let drawn = screen(&mut shell, 100, 30);
        assert!(drawn.contains(marker), "{view:?}");
        assert!(!drawn.contains("Seed"), "nothing of the game shows");
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("backend");
        ui::render(&mut terminal, &mut shell).expect("draw");
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|c| c.fg == ratatui::style::Color::Reset
                    && c.bg == ratatui::style::Color::Reset),
            "no colours"
        );
        // Any key comes back, and does nothing else.
        press(&mut shell, "1");
        assert!(!shell.boss());
        assert_eq!(app(&shell).digest(), before);
    }
    // The keys are configurable.
    std::fs::write(dirs.settings(), "boss_keys = [\"F9\"]\n").expect("write");
    let mut shell = launch(&dirs);
    shell.handle_key(KeyCode::F(12));
    assert!(!shell.boss());
    shell.handle_key(KeyCode::F(9));
    assert!(shell.boss());
}

/// Every key worth trying.
fn all_keys() -> Vec<KeyCode> {
    let mut keys: Vec<KeyCode> = (b' '..=b'~')
        .map(|b| KeyCode::Char(char::from(b)))
        .collect();
    keys.extend((1..=12).map(KeyCode::F));
    keys.extend([
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Backspace,
        KeyCode::Tab,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::Delete,
        KeyCode::Insert,
    ]);
    keys
}

/// Every key that does something on `shell`'s screen is in its help, and the help draws it.
fn assert_help_covers(shell: &Shell, what: &str) {
    let sections = ui::help::sections(shell);
    let listed: Vec<String> = sections
        .iter()
        .flat_map(|(_, rows)| rows.iter().map(|(label, _)| label.clone()))
        .collect();
    let boss = &shell.settings().boss_keys;
    for key in all_keys() {
        let Some((_, binding)) = keys::lookup(&shell.layers(), key, boss) else {
            continue;
        };
        if matches!(binding.command, Command::Ignore | Command::Unknown) {
            continue;
        }
        let label = if binding.keys == Keys::Boss {
            shell.settings().boss_key_names()
        } else {
            binding.label.to_owned()
        };
        assert!(
            !label.is_empty() && listed.contains(&label),
            "{what}: {key:?} ({:?}) is not in the help {listed:?}",
            binding.command
        );
    }
    if shell
        .layers()
        .first()
        .is_some_and(|l| l.title == keys::SHELL.title && l.bindings.len() > 1)
    {
        let mut open = shell.clone();
        open.handle_key(KeyCode::Char('?'));
        assert_eq!(open.panel(), Some(Panel::Help(0)), "{what}");
        // The whole help, page by page: the column is short of rows for it, so it scrolls.
        let mut drawn = String::new();
        let mut pages = 0;
        loop {
            let page: String = screen(&mut open, 200, 60).split_whitespace().collect();
            drawn.push_str(&page);
            open.handle_key(KeyCode::PageDown);
            pages += 1;
            if screen(&mut open, 200, 60)
                .split_whitespace()
                .collect::<String>()
                == page
                || pages > 20
            {
                break;
            }
        }
        for label in &listed {
            let label: String = label.split_whitespace().collect();
            assert!(drawn.contains(&label), "{what}: {label} is not drawn");
        }
        open.handle_key(KeyCode::Char('x'));
        assert_eq!(open.panel(), None, "any other key closes the help");
    }
}

#[test]
fn the_help_lists_every_key_of_every_screen() {
    // Every binding that does something can be listed.
    for layer in keys::ALL {
        for b in layer.bindings {
            assert!(
                b.label.is_empty() == matches!(b.command, Command::Ignore | Command::Unknown),
                "{}: {b:?}",
                layer.title
            );
        }
    }
    let dirs = scratch("help");
    let mut shell = launch(&dirs);
    assert_help_covers(&shell, "select");
    press(&mut shell, "s");
    assert_help_covers(&shell, "typing a seed");
    // A fixed seed: the run below must reach the same screens on every machine. A random seed
    // made this test depend on where the run happened to end (a frozen gate run once ended
    // before a confirm prompt).
    press(&mut shell, "RL2S0");
    shell.handle_key(KeyCode::Enter);
    shell.handle_key(KeyCode::Enter);
    assert_help_covers(&shell, "Neow");
    let mut continued = launch(&dirs);
    assert_help_covers(&continued, "continue");
    std::fs::write(dirs.crash(), "x").expect("note");
    continued = launch(&dirs);
    assert_help_covers(&continued, "continue after a crash");
    press(&mut shell, "d");
    assert_help_covers(&shell, "overlay");
    press(&mut shell, "d");
    // Into a fight, then a selected card, a target pick and a question.
    let mut rng = 7;
    let mut seen = Vec::new();
    for _ in 0..3000 {
        let a = app(&shell);
        let state = match (
            a.combat().map(sts2_core::Combat::status),
            a.input(),
            a.confirm_prompt(),
        ) {
            (_, _, Some(_)) => "question",
            (Some(Status::PlayerTurn), Input::Idle, _) => "turn",
            (Some(_), Input::Target(_), _) => "target",
            (None, _, _) if a.is_over() => "end",
            (None, _, _) => "menu",
            _ => "other",
        };
        if !seen.contains(&state) {
            seen.push(state);
            assert_help_covers(&shell, state);
            if state == "turn" {
                let card = a
                    .combat()
                    .and_then(|c| c.state().player.hand.first().copied());
                if let Some(card) = card {
                    let mut armed = shell.clone();
                    armed.perform(UiAction::Arm(card));
                    assert_help_covers(&armed, "selected card");
                }
            }
        }
        if a.is_over() {
            break;
        }
        // Leaving a reward behind asks first.
        let enabled: Vec<_> = a.menu().into_iter().filter(|i| i.enabled).collect();
        if let Some(n) = enabled.iter().position(|i| i.warning.is_some())
            && !seen.contains(&"question")
        {
            press(&mut shell, &(n + 1).to_string());
            continue;
        }
        random_input(&mut shell, &mut rng);
    }
    for state in ["turn", "menu", "question"] {
        assert!(seen.contains(&state), "never saw {state}: {seen:?}");
    }
}

#[test]
fn the_main_menu_shows_with_a_save_and_each_entry_does_its_thing_by_mouse() {
    let dirs = scratch("menu-mouse");
    let actions = play_and_quit(&dirs);
    let mut shell = launch(&dirs);
    let Mode::Select(select) = shell.mode() else {
        panic!("select");
    };
    assert!(select.continue_focused(), "the main menu shows");
    assert!(screen(&mut shell, 150, 46).contains("Continue"));

    click(&mut shell, Target::Button(UiAction::Settings));
    assert_eq!(shell.panel(), Some(Panel::Settings(0)));
    shell.handle_key(KeyCode::Esc);
    assert_eq!(shell.panel(), None);
    click(&mut shell, Target::Button(UiAction::Continue));
    assert_eq!(app(&shell).actions(), actions, "Continue resumes the run");

    let mut shell = launch(&dirs);
    click(&mut shell, Target::Button(UiAction::NewRun));
    let Mode::Select(select) = shell.mode() else {
        panic!("select");
    };
    assert!(!select.continue_focused(), "New run: the character select");

    let mut shell = launch(&dirs);
    click(&mut shell, Target::Button(UiAction::Quit));
    assert!(shell.should_quit());
}

#[test]
fn a_pointer_resting_on_another_entry_leaves_enter_on_continue() {
    let dirs = scratch("menu-hover");
    let actions = play_and_quit(&dirs);
    for entry in [UiAction::NewRun, UiAction::Settings, UiAction::Quit] {
        let mut shell = launch(&dirs);
        hover(&mut shell, Target::Button(entry));
        let Mode::Select(select) = shell.mode() else {
            panic!("select");
        };
        assert_eq!(
            select.focused(),
            Some(Target::Button(UiAction::Continue)),
            "hovering {entry:?} does not move the focus"
        );
        shell.handle_key(KeyCode::Enter);
        assert_eq!(app(&shell).actions(), actions, "Enter continues the run");
    }
}

#[test]
fn side_arrows_and_a_right_click_stay_on_the_main_menu() {
    let dirs = scratch("menu-side");
    let actions = play_and_quit(&dirs);
    let menu_focus = |shell: &Shell| {
        let Mode::Select(select) = shell.mode() else {
            panic!("select");
        };
        assert!(select.continue_focused(), "still the main menu");
        select.focused()
    };

    let mut shell = launch(&dirs);
    shell.handle_key(KeyCode::Right);
    assert_eq!(menu_focus(&shell), Some(Target::Button(UiAction::NewRun)));
    shell.handle_key(KeyCode::Left);
    assert_eq!(menu_focus(&shell), Some(Target::Button(UiAction::Continue)));
    shell.handle_key(KeyCode::Left);
    assert_eq!(
        menu_focus(&shell),
        Some(Target::Button(UiAction::Quit)),
        "wraps"
    );
    shell.handle_key(KeyCode::Right);
    shell.handle_key(KeyCode::Enter);
    assert_eq!(app(&shell).actions(), actions, "Enter continues the run");

    let mut shell = launch(&dirs);
    mouse(
        &mut shell,
        MouseEventKind::Down(MouseButton::Right),
        Target::Button(UiAction::Continue),
    );
    assert_eq!(menu_focus(&shell), Some(Target::Button(UiAction::Continue)));
    shell.handle_key(KeyCode::Enter);
    assert_eq!(app(&shell).actions(), actions, "Enter continues the run");
}

#[test]
fn mouse_play_is_saved_and_continue_resumes_it() {
    let dirs = scratch("mouse-play");
    let mut shell = launch(&dirs);
    shell.handle_key(KeyCode::Enter);
    // Through the opening choice and its rewards to the map, clicking the first option on
    // every screen, then the first map node drawn.
    let mut node = None;
    for _ in 0..12 {
        let _ = screen(&mut shell, 150, 46);
        let reachable: Vec<MapCoord> = app(&shell)
            .run()
            .legal_actions()
            .into_iter()
            .filter_map(|a| match a {
                RunAction::ChooseNode(c) => Some(c),
                _ => None,
            })
            .collect();
        let first = shell.hits().regions().iter().find_map(|r| match r.target {
            Target::MapNode(c) if reachable.contains(&c) => Some(r.target),
            Target::Button(UiAction::Pick(_)) => Some(r.target),
            _ => None,
        });
        match first {
            Some(target @ Target::MapNode(coord)) => {
                click(&mut shell, target);
                node = Some(coord);
                break;
            }
            Some(target) => click(&mut shell, target),
            None if app(&shell).confirm_prompt().is_some() => press(&mut shell, "y"),
            None => shell.handle_key(KeyCode::Enter),
        }
        assert_save_restores(&dirs, &shell);
    }
    let node = node.expect("the map");
    assert_eq!(app(&shell).run().state().coord(), Some(node));
    let mut rng = 11;
    for _ in 0..4 {
        random_input(&mut shell, &mut rng);
    }
    let actions = app(&shell).actions().to_vec();
    assert!(actions.len() >= 2);
    assert_save_restores(&dirs, &shell);
    let run = app(&shell).run().clone();
    press(&mut shell, "q");

    let mut again = launch(&dirs);
    click(&mut again, Target::Button(UiAction::Continue));
    assert_eq!(app(&again).actions(), actions);
    assert_eq!(*app(&again).run(), run);
    // The next action is saved on top of the restored run.
    random_input(&mut again, &mut rng);
    assert_save_restores(&dirs, &again);
}
