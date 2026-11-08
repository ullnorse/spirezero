//! `cargo run -p sts2-tui -- [SEED] [--character NAME] [--ascension N] [--no-mouse] [--theme
//! dark|light|muted]`: with no
//! run arguments the main menu opens, offering the saved run as Continue, or the character
//! select; with any, the run starts at once (Necrobinder by default, a random seed unless one
//! is given). The run is saved after every action, so `q` quits at once. The mouse works unless
//! `--no-mouse` leaves it to the terminal (to select text, say), for one session over the
//! settings (`o`, `settings.toml`). The colours follow the terminal's `COLORFGBG` (a light
//! background gets the light theme) until the theme setting is changed; `--theme` overrides
//! both for the session. `--startup-time` draws the first screen, quits and prints how long
//! that took.

use std::io::{self, Write as _};
use std::rc::Rc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use sts2_core::Character;
use sts2_tui::app::{App, random_seed};
use sts2_tui::session::{self, Dirs, Session};
use sts2_tui::settings::{Settings, Theme};
use sts2_tui::shell::Shell;
use sts2_tui::{names, ui};

const USAGE: &str = "usage: sts2-tui [SEED] [--character NAME] [--ascension N] [--no-mouse] [--theme dark|light|muted]";

/// The command line.
#[derive(Default)]
struct Args {
    seed: Option<String>,
    character: Option<Character>,
    ascension: Option<u8>,
    mouse: Option<bool>,
    theme: Option<Theme>,
    startup_time: bool,
}

impl Args {
    fn parse() -> io::Result<Self> {
        let mut parsed = Self::default();
        let mut args = std::env::args().skip(1);
        let usage = || io::Error::other(USAGE);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--character" => {
                    parsed.character = Some(
                        args.next()
                            .and_then(|name| Character::from_name(&name))
                            .ok_or_else(usage)?,
                    );
                }
                "--ascension" => {
                    parsed.ascension =
                        Some(args.next().and_then(|n| n.parse().ok()).ok_or_else(usage)?);
                }
                "--no-mouse" => parsed.mouse = Some(false),
                "--theme" => {
                    parsed.theme = Some(
                        args.next()
                            .and_then(|name| Theme::from_name(&name))
                            .ok_or_else(usage)?,
                    );
                }
                "--startup-time" => parsed.startup_time = true,
                _ if parsed.seed.is_none() && !arg.starts_with('-') => parsed.seed = Some(arg),
                _ => return Err(usage()),
            }
        }
        Ok(parsed)
    }

    /// Whether the command line names a run to start at once.
    const fn names_a_run(&self) -> bool {
        self.seed.is_some() || self.character.is_some() || self.ascension.is_some()
    }
}

/// The command line's overrides of the settings, kept until the setting itself is changed.
struct Overrides {
    mouse: Option<bool>,
    /// The settings as loaded, to tell when one changed.
    loaded: Settings,
}

impl Overrides {
    const fn mouse(&self, settings: &Settings) -> bool {
        match self.mouse {
            Some(m) if settings.mouse == self.loaded.mouse => m,
            _ => settings.mouse,
        }
    }
}

fn main() -> io::Result<()> {
    let started = Instant::now();
    let args = Args::parse()?;
    let dirs = Dirs::discover();
    let settings = dirs
        .as_ref()
        .map(|d| Settings::load(&d.settings()))
        .unwrap_or_default();
    let overrides = Overrides {
        mouse: args.mouse,
        loaded: settings.clone(),
    };
    let text = Rc::new(sts2_tui::text::discover());
    let app = if args.names_a_run() {
        let seed = args.seed.clone().unwrap_or_else(random_seed);
        Some(
            App::with_text(
                args.character.unwrap_or_default(),
                &seed,
                args.ascension.unwrap_or(0),
                Rc::clone(&text),
            )
            .map_err(|e| {
                io::Error::other(format!("cannot start the run: {}", names::run_error(&e)))
            })?,
        )
    } else {
        None
    };
    let mut shell = Shell::launch(text, settings, dirs.clone().map(Session::new), app);
    if let Some(theme) = args.theme.or_else(|| {
        std::env::var("COLORFGBG")
            .ok()
            .and_then(|v| Theme::from_colorfgbg(&v))
    }) {
        shell.set_default_theme(theme);
    }
    // `init` restores the screen and raw mode on a panic. Before it does, the hook below turns
    // the mouse off (or the terminal would go on printing mouse sequences into the shell) and
    // writes the crash note next to the save, which Continue then picks up from.
    let mut terminal = ratatui::init();
    install_panic_hook(dirs);
    let mut mouse = false;
    let result = play(
        &mut terminal,
        &mut shell,
        &args,
        &overrides,
        started,
        &mut mouse,
    );
    // Whatever went wrong inside, hand the shell back as it was: mouse off, screen restored.
    let _ = set_mouse(&mut mouse, false);
    ratatui::restore();
    let first_frame = result?;
    if args.startup_time {
        writeln!(io::stdout(), "first frame {} ms", first_frame.as_millis())?;
    }
    Ok(())
}

/// The first frame and the event loop; any I/O error ends it early. Returns how long the
/// first frame took.
fn play(
    terminal: &mut ratatui::DefaultTerminal,
    shell: &mut Shell,
    args: &Args,
    overrides: &Overrides,
    started: Instant,
    mouse: &mut bool,
) -> io::Result<Duration> {
    ui::theme::set_true_color(
        std::env::var("COLORTERM").is_ok_and(|c| matches!(c.as_str(), "truecolor" | "24bit")),
    );
    ui::render(terminal, shell)?;
    let first_frame = started.elapsed();
    if !args.startup_time {
        set_mouse(mouse, overrides.mouse(shell.settings()))?;
        run(terminal, shell, overrides, mouse)?;
    }
    Ok(first_frame)
}

/// Turns mouse capture on or off, if it is not already.
fn set_mouse(current: &mut bool, on: bool) -> io::Result<()> {
    if *current != on {
        if on {
            execute!(io::stdout(), EnableMouseCapture)?;
        } else {
            execute!(io::stdout(), DisableMouseCapture)?;
        }
        *current = on;
    }
    Ok(())
}

/// On a panic the game cannot recover from: the mouse off, a crash note beside the save, then
/// the hook `ratatui::init` set (the terminal restored, the message printed).
fn install_panic_hook(dirs: Option<Dirs>) {
    let restore = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if session::panic_is_caught() {
            return;
        }
        let _ = execute!(io::stdout(), DisableMouseCapture);
        if let Some(dirs) = &dirs {
            session::write_crash_note(dirs, info);
        }
        restore(info);
        if let Some(dirs) = &dirs {
            let _ = writeln!(
                io::stderr(),
                "The game crashed; the run is saved up to the last good action (Continue picks it up).\nCrash note: {}",
                dirs.crash().display()
            );
        }
    }));
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    shell: &mut Shell,
    overrides: &Overrides,
    mouse: &mut bool,
) -> io::Result<()> {
    while !shell.should_quit() {
        ui::motion::set_enabled(shell.settings().motion);
        shell.poll_hint();
        ui::render(terminal, shell)?;
        // While something moves or the bot thinks, look again on a tick; else wait for input.
        if (ui::motion::active() || shell.thinking()) && !event::poll(ui::motion::TICK)? {
            continue;
        }
        // Handle everything already waiting (a mouse move sends many events) before drawing.
        let mut next = Some(event::read()?);
        while let Some(event) = next.take() {
            // A key finishes whatever moves at once; the key itself still counts.
            if matches!(event, Event::Key(_)) {
                ui::motion::finish();
            }
            if !handle(shell, &event) || shell.should_quit() {
                return Ok(());
            }
            if shell.take_settings_changed() {
                set_mouse(mouse, overrides.mouse(shell.settings()))?;
            }
            // A click or key may change the screen; what the next event points at (a position
            // in a list) is only meaningful on the frame drawn after it.
            if !is_pointer_motion(&event) {
                break;
            }
            if event::poll(Duration::ZERO)? {
                next = Some(event::read()?);
            }
        }
    }
    Ok(())
}

/// A mouse move or drag, which never changes the screen by itself.
const fn is_pointer_motion(event: &Event) -> bool {
    matches!(event, Event::Mouse(m) if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)))
}

/// Handles one terminal event; `false` means quit at once.
fn handle(shell: &mut Shell, event: &Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            // Raw mode turns Ctrl-C into a key press: quit like a terminal program would.
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                return false;
            }
            shell.handle_key(key.code);
        }
        Event::Mouse(mouse) => shell.handle_mouse(*mouse),
        _ => {}
    }
    true
}
