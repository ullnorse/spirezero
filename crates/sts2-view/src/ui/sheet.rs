//! The run menu and the lists behind it as client state: which sheet is open, how far it is
//! scrolled, and what a tap, a key or Back means on it.

use sts2_core::run::{MapCoord, Run, RunAction};

use super::{Effect, Ui, done, pass};
use crate::describe::Describer;
use crate::model::{Block, ButtonId, Input, Key, Screen};
use crate::sheets::{self, Context, Entry, Kind};

/// Rows a page step scrolls.
const PAGE_ROWS: isize = 10;

/// The sheet on show.
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    open: Option<Kind>,
    /// Rows from the sheet's first line, or from its last for a sheet read from the bottom.
    scroll: usize,
}

impl Ui {
    /// The sheet open over the screen.
    #[must_use]
    pub const fn sheet(&self) -> Option<Kind> {
        self.sheet.open
    }

    /// How many rows the open sheet is scrolled: from its top, or from its newest line for
    /// the log.
    #[must_use]
    pub const fn sheet_scroll(&self) -> usize {
        self.sheet.scroll
    }

    /// Scrolls the open sheet `by` rows toward its end (negative: toward its start).
    pub fn scroll_sheet(&mut self, by: isize) {
        let from_bottom = self.sheet.open.is_some_and(Kind::from_bottom);
        let by = if from_bottom { -by } else { by };
        self.sheet.scroll = self.sheet.scroll.saturating_add_signed(by);
    }

    /// Keeps the scroll within `max` rows (what the last frame could show).
    pub fn clamp_sheet_scroll(&mut self, max: usize) {
        self.sheet.scroll = self.sheet.scroll.min(max);
    }

    /// Opens `kind` (scrolled to its start), or an error when it is a fight's and there is no
    /// fight.
    pub fn open_sheet(&mut self, run: &Run, kind: Kind) -> Vec<Effect> {
        if kind.fight_only() && run.combat().is_none() {
            let what = if kind == Kind::Log {
                "the log exists"
            } else {
                "the piles exist"
            };
            return vec![Effect::Error(format!("{what} only in a fight"))];
        }
        self.sheet = State {
            open: Some(kind),
            scroll: 0,
        };
        done()
    }

    /// Opens `kind`, or closes it when it is the one open.
    pub fn toggle_sheet(&mut self, run: &Run, kind: Kind) -> Vec<Effect> {
        if self.sheet.open == Some(kind) {
            self.close_sheet();
            return done();
        }
        self.open_sheet(run, kind)
    }

    pub const fn close_sheet(&mut self) {
        self.sheet = State {
            open: None,
            scroll: 0,
        };
    }

    /// The open sheet as a screen; `None` with no sheet open (or a fight's outside a fight).
    #[must_use]
    pub fn sheet_screen(&self, ctx: &Context<'_>) -> Option<Screen> {
        sheets::screen(self.sheet.open?, ctx)
    }

    /// Handles `input` while a sheet is open: Back or Esc closes it, a tap or a key on the
    /// menu does its row, a tap on a reachable node of the map travels there, the arrows and
    /// pages scroll. Everything else is swallowed.
    pub fn sheet_input(&mut self, run: &Run, describer: &Describer, input: &Input) -> Vec<Effect> {
        let Some(kind) = self.sheet.open else {
            return pass();
        };
        let ctx = Context {
            run,
            describer,
            visits: &[],
            tossing: None,
        };
        match *input {
            Input::Back | Input::Key(Key::Esc | Key::Backspace) => {
                self.close_sheet();
                done()
            }
            Input::Page(n) => {
                self.scroll_sheet(isize::try_from(n).unwrap_or(0).saturating_mul(PAGE_ROWS));
                done()
            }
            Input::Key(Key::PageUp) => self.scrolled(-PAGE_ROWS),
            Input::Key(Key::PageDown) => self.scrolled(PAGE_ROWS),
            Input::Key(Key::Up) => self.scrolled(-1),
            Input::Key(Key::Down) => self.scrolled(1),
            Input::Tap(id) => match kind {
                Kind::Menu => sheets::entry_of(&ctx, id).map_or_else(done, |e| self.entry(run, e)),
                Kind::Map => self.travel(run, describer, id),
                _ => done(),
            },
            Input::Key(Key::Char(c)) => sheets::menu_entries(run.combat().is_some())
                .into_iter()
                .find(|e| e.key == c)
                .map_or_else(done, |e| self.entry(run, e.entry)),
            Input::Key(Key::Digit(d)) if kind == Kind::Menu => {
                let slot = if d == 0 { 9 } else { usize::from(d) - 1 };
                self.entry(run, Entry::Toss(slot))
            }
            _ => done(),
        }
    }

    fn scrolled(&mut self, by: isize) -> Vec<Effect> {
        self.scroll_sheet(by);
        done()
    }

    /// A menu row done: a sheet opens, the rest is the client's.
    fn entry(&mut self, run: &Run, entry: Entry) -> Vec<Effect> {
        match entry {
            Entry::Sheet(kind) => self.toggle_sheet(run, kind),
            other => vec![Effect::Menu(other)],
        }
    }

    /// A tap on a node of the enlarged map: travel there, closing the sheet.
    fn travel(&mut self, run: &Run, describer: &Describer, id: ButtonId) -> Vec<Effect> {
        let ctx = Context {
            run,
            describer,
            visits: &[],
            tossing: None,
        };
        let Some(screen) = sheets::screen(Kind::Map, &ctx) else {
            return done();
        };
        let cell = screen.blocks.iter().find_map(|b| match b {
            Block::Map(grid) => grid.cells.iter().find(|c| c.button == Some(id)),
            _ => None,
        });
        let Some(cell) = cell else {
            return done();
        };
        let action = RunAction::ChooseNode(MapCoord {
            col: usize::from(cell.x),
            row: usize::from(cell.y),
        });
        if run.legal_actions().contains(&action) {
            self.close_sheet();
            vec![Effect::Apply(action)]
        } else {
            done()
        }
    }
}
