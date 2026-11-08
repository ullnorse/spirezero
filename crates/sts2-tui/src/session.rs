//! Saving the run in progress and finding it again: the autosave, Continue, and the crash note.
//!
//! A save is the run's seed, character and ascension and the list of actions taken (each as its
//! index in [`App::recordable_actions`]), written after every action: a few kilobytes for a
//! whole run. Continue plays the list back and checks the result against the digest the save
//! stores ([`App::digest`]). The file lives in the platform's data directory ([`Dirs`]); the
//! settings live in its config directory.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use sts2_core::Character;

use crate::app::{App, ReplayError};
use crate::text::Localization;

/// Overrides both directories (one directory for everything: a portable install, a test).
pub const HOME_VAR: &str = "SPIREZERO_HOME";
/// The project's old name for [`HOME_VAR`], still read when the new one is unset.
const OLD_HOME_VAR: &str = "SPIREBOT_HOME";

/// The first line of a save; the number goes up when the format changes.
const MAGIC: &str = "spirezero-save";
/// The magic of saves written under the project's old name; they still load.
const OLD_MAGIC: &str = "spirebot-save";

/// The directory name under the platform's data and config dirs.
const APP: &str = "spirezero";
/// The old [`APP`]: used while it exists and the new one doesn't, so old saves stay found.
const OLD_APP: &str = "spirebot";

/// `base/spirezero`, or `base/spirebot` when only that one exists.
fn app_dir(base: &Path) -> PathBuf {
    let new = base.join(APP);
    let old = base.join(OLD_APP);
    if !new.exists() && old.is_dir() {
        old
    } else {
        new
    }
}
pub const VERSION: u32 = 1;

/// Where the save, the crash note and the settings are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dirs {
    /// The save and crash notes.
    pub data: PathBuf,
    /// `settings.toml`.
    pub config: PathBuf,
}

impl Dirs {
    /// Everything in `dir`.
    #[must_use]
    pub fn at(dir: &Path) -> Self {
        Self {
            data: dir.to_path_buf(),
            config: dir.to_path_buf(),
        }
    }

    /// `$SPIREZERO_HOME` (or the old `$SPIREBOT_HOME`) if set; else the platform's:
    /// `%APPDATA%\spirezero` on Windows, `~/Library/Application Support/spirezero` on macOS, and
    /// `$XDG_DATA_HOME/spirezero` (`~/.local/share/spirezero`) with `$XDG_CONFIG_HOME/spirezero`
    /// (`~/.config/spirezero`) elsewhere. Each falls back to the old `spirebot` directory when
    /// only that one exists. `None` without a home directory.
    #[must_use]
    pub fn discover() -> Option<Self> {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        if let Some(home) = var(HOME_VAR).or_else(|| var(OLD_HOME_VAR)) {
            return Some(Self::at(&home));
        }
        if cfg!(windows) {
            let base = var("APPDATA").or_else(|| var("USERPROFILE"))?;
            return Some(Self::at(&app_dir(&base)));
        }
        let home = var("HOME")?;
        if cfg!(target_os = "macos") {
            return Some(Self::at(&app_dir(
                &home.join("Library/Application Support"),
            )));
        }
        Some(Self {
            data: app_dir(&var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"))),
            config: app_dir(&var("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))),
        })
    }

    #[must_use]
    pub fn save(&self) -> PathBuf {
        self.data.join("save.txt")
    }

    /// The note the panic hook writes; renamed to [`Dirs::old_crash`] once shown.
    #[must_use]
    pub fn crash(&self) -> PathBuf {
        self.data.join("crash.txt")
    }

    #[must_use]
    pub fn old_crash(&self) -> PathBuf {
        self.data.join("crash-last.txt")
    }

    #[must_use]
    pub fn settings(&self) -> PathBuf {
        self.config.join("settings.toml")
    }
}

/// Writes `bytes` to `path` through a temporary file beside it, so the file is always whole:
/// the old one or the new one.
///
/// # Errors
/// If the directory cannot be made or the file written.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// A run in progress as the save file holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save {
    pub character: Character,
    pub ascension: u8,
    pub seed: String,
    /// Where the run is (act 1-based), and the HP and gold there: for the Continue line,
    /// without playing the run back.
    pub act: usize,
    pub floor: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    /// [`App::digest`] after the last action, and before it (when known).
    pub digest: u64,
    pub previous: Option<u64>,
    pub actions: Vec<u32>,
}

/// What [`Save::restore`] found.
#[derive(Debug)]
pub struct Restored {
    pub app: App,
    /// Whether the replay reached the state the save was made in. `false` means the engine
    /// changed since: the run goes on, but it may differ from the one saved.
    pub exact: bool,
}

impl Save {
    /// The save of `app`'s run; `previous` is the digest before its last action, if known.
    #[must_use]
    pub fn of(app: &App, previous: Option<u64>) -> Self {
        let state = app.run().state();
        let hp = app.combat().map_or(state.hp, |c| {
            c.state().creature(c.state().player.creature).hp.max(0)
        });
        Self {
            character: state.character,
            ascension: state.ascension,
            seed: app.seed().to_owned(),
            act: state.act_index + 1,
            floor: state.history.len(),
            hp,
            max_hp: state.max_hp,
            gold: state.gold,
            digest: app.digest(),
            previous,
            actions: app.actions().to_vec(),
        }
    }

    /// `Necrobinder  A0  Act 1 floor 5  HP 50/70  99 gold`.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}  A{}  Act {} floor {}  HP {}/{}  {} gold",
            self.character.name(),
            self.ascension,
            self.act,
            self.floor,
            self.hp,
            self.max_hp,
            self.gold
        )
    }

    /// The save as its file holds it.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "{MAGIC} {VERSION}\ncharacter {}\nascension {}\nseed {}\nplace {} {}\nhp {} {}\ngold {}\ndigest {:016x}",
            self.character.name(),
            self.ascension,
            self.seed,
            self.act,
            self.floor,
            self.hp,
            self.max_hp,
            self.gold,
            self.digest,
        );
        if let Some(previous) = self.previous {
            let _ = write!(out, " {previous:016x}");
        }
        out.push_str("\nactions");
        for a in &self.actions {
            let _ = write!(out, " {a}");
        }
        out.push('\n');
        out
    }

    /// Reads a save file.
    ///
    /// # Errors
    /// If it is not a save of this format version or a line is missing or bad.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header = lines.next().unwrap_or("");
        if header != format!("{MAGIC} {VERSION}") && header != format!("{OLD_MAGIC} {VERSION}") {
            return Err(format!("not a version {VERSION} save ({header:?})"));
        }
        let mut fields = std::collections::HashMap::new();
        for line in lines {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            fields.insert(key, value);
        }
        let field = |key: &str| {
            fields
                .get(key)
                .copied()
                .ok_or_else(|| format!("no {key} line"))
        };
        let bad = |key: &str| format!("bad {key} line");
        let numbers = |key: &str| -> Result<Vec<i64>, String> {
            field(key)?
                .split_whitespace()
                .map(|n| n.parse().map_err(|_| bad(key)))
                .collect()
        };
        let pair = |key: &str| -> Result<(i64, i64), String> {
            match numbers(key)?.as_slice() {
                &[a, b] => Ok((a, b)),
                _ => Err(bad(key)),
            }
        };
        let int = |n: i64, key: &str| i32::try_from(n).map_err(|_| bad(key));
        let index = |n: i64, key: &str| usize::try_from(n).map_err(|_| bad(key));
        let (act, floor) = pair("place")?;
        let (hp, max_hp) = pair("hp")?;
        let mut digests = field("digest")?
            .split_whitespace()
            .map(|d| u64::from_str_radix(d, 16).map_err(|_| bad("digest")));
        Ok(Self {
            character: Character::from_name(field("character")?).ok_or_else(|| bad("character"))?,
            ascension: field("ascension")?.parse().map_err(|_| bad("ascension"))?,
            seed: field("seed")?.to_owned(),
            act: index(act, "place")?,
            floor: index(floor, "place")?,
            hp: int(hp, "hp")?,
            max_hp: int(max_hp, "hp")?,
            gold: field("gold")?.parse().map_err(|_| bad("gold"))?,
            digest: digests.next().ok_or_else(|| bad("digest"))??,
            previous: digests.next().transpose()?,
            actions: field("actions")?
                .split_whitespace()
                .map(|n| n.parse().map_err(|_| bad("actions")))
                .collect::<Result<_, _>>()?,
        })
    }

    /// The save one action earlier (Continue after a crash the last action caused).
    #[must_use]
    pub fn earlier(&self) -> Option<Self> {
        let (_, actions) = self.actions.split_last()?;
        Some(Self {
            actions: actions.to_vec(),
            digest: self.previous?,
            previous: None,
            ..self.clone()
        })
    }

    /// Plays the run back, showing `text`.
    ///
    /// # Errors
    /// If the run cannot start or an action no longer applies (the engine changed).
    pub fn restore(&self, text: Rc<Localization>) -> Result<Restored, ReplayError> {
        let app = App::replay(
            self.character,
            &self.seed,
            self.ascension,
            text,
            &self.actions,
        )?;
        let exact = app.digest() == self.digest;
        Ok(Restored { app, exact })
    }
}

/// The autosave: keeps the save file in step with the run shown.
#[derive(Clone, Debug)]
pub struct Session {
    dirs: Dirs,
    /// The length of the action list and the digest last written.
    written: Option<(usize, u64)>,
}

impl Session {
    #[must_use]
    pub const fn new(dirs: Dirs) -> Self {
        Self {
            dirs,
            written: None,
        }
    }

    #[must_use]
    pub const fn dirs(&self) -> &Dirs {
        &self.dirs
    }

    /// The saved run, if there is one that can be read.
    #[must_use]
    pub fn load(&self) -> Option<Save> {
        let text = std::fs::read_to_string(self.dirs.save()).ok()?;
        Save::parse(&text).ok()
    }

    /// The crash note the last session left, if it has not been shown yet; it is moved aside
    /// so it is shown once.
    #[must_use]
    pub fn take_crash_note(&self) -> Option<PathBuf> {
        let note = self.dirs.crash();
        if !note.exists() {
            return None;
        }
        let old = self.dirs.old_crash();
        Some(if std::fs::rename(&note, &old).is_ok() {
            old
        } else {
            note
        })
    }

    /// A new run starts: the next write has no action before it to know.
    pub const fn forget(&mut self) {
        self.written = None;
    }

    /// Notes that `app` was just restored from the save on disk (or started), so the next
    /// write knows the digest before its action.
    pub fn resume(&mut self, app: &App) {
        self.written = Some((app.actions().len(), app.digest()));
    }

    /// Brings the save up to date with `app`: writes it, or deletes it once the run is over.
    ///
    /// # Errors
    /// If the file cannot be written or deleted.
    pub fn autosave(&mut self, app: &App) -> io::Result<()> {
        if app.is_over() {
            self.written = None;
            return self.run_ended(app);
        }
        let len = app.actions().len();
        let previous = self
            .written
            .and_then(|(n, digest)| (n + 1 == len).then_some(digest));
        let save = Save::of(app, previous);
        write_atomic(&self.dirs.save(), save.to_text().as_bytes())?;
        self.written = Some((len, save.digest));
        Ok(())
    }

    /// The run ended (died, won, or stopped on content that is not ported): the save goes.
    /// The hook for keeping a history of runs.
    ///
    /// # Errors
    /// If the save exists and cannot be deleted.
    pub fn run_ended(&self, _app: &App) -> io::Result<()> {
        match std::fs::remove_file(self.dirs.save()) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

thread_local! {
    static CATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f`, catching a panic in it; the panic hook leaves such a panic alone
/// ([`panic_is_caught`]): the terminal stays as it is and no crash note is written.
///
/// # Errors
/// The panic's payload, if `f` panicked.
pub fn catch_panic<T>(f: impl FnOnce() -> T) -> std::thread::Result<T> {
    let before = CATCHING.replace(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CATCHING.set(before);
    result
}

/// Whether the panic being handled happens inside [`catch_panic`].
#[must_use]
pub fn panic_is_caught() -> bool {
    CATCHING.get()
}

/// Writes the crash note for a panic: what happened, where, and the backtrace.
pub fn write_crash_note(dirs: &Dirs, info: &std::panic::PanicHookInfo<'_>) {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned());
    let place = info
        .location()
        .map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
    let saved = std::fs::read_to_string(dirs.save())
        .ok()
        .and_then(|t| Save::parse(&t).ok())
        .map_or_else(
            || "no run was saved".to_owned(),
            |s| {
                format!(
                    "the run is saved after {} actions: {}",
                    s.actions.len(),
                    s.summary()
                )
            },
        );
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let note = format!(
        "sts2-tui {} crashed (unix time {seconds}): {message}{place}\n{saved}\n\n{}\n",
        env!("CARGO_PKG_VERSION"),
        std::backtrace::Backtrace::force_capture()
    );
    let _ = write_atomic(&dirs.crash(), note.as_bytes());
}
