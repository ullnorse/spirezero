//! The game's text in the terminal: where the localization tables come from, and how
//! [`Rich`](sts2_view::text::Rich) becomes ratatui text.
//!
//! The text itself, its markup and its roles live in `sts2_view::text`; this module only loads
//! the JSON files (or the copy an `embedded` build carries) and maps a role to the terminal's
//! palette. The TUI never sees a colour in a game string.

use std::path::{Path, PathBuf};

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

pub use sts2_view::text::{Localization, Rich, RichSpan, Role, TABLES, Value, format, plain};

use crate::ui::theme;

/// The environment variable that names the localization directory.
pub const ENV_VAR: &str = "STS2_LOCALIZATION";

/// Reads the [`TABLES`] from `dir`; a missing or malformed file is skipped.
#[must_use]
pub fn from_dir(dir: &Path) -> Localization {
    let tables: Vec<(&str, String)> = TABLES
        .iter()
        .filter_map(|table| {
            std::fs::read_to_string(dir.join(format!("{table}.json")))
                .ok()
                .map(|json| (*table, json))
        })
        .collect();
    let pairs: Vec<(&str, &str)> = tables
        .iter()
        .map(|(table, json)| (*table, json.as_str()))
        .collect();
    Localization::from_json(&pairs)
}

/// The first of [`candidates`] that holds at least one table, or an empty localization. The
/// [`built_in`] text comes right after `$STS2_LOCALIZATION`.
#[must_use]
pub fn discover() -> Localization {
    let dirs = candidates();
    let (env, others) = dirs.split_at(usize::from(std::env::var_os(ENV_VAR).is_some()));
    env.iter()
        .map(|dir| from_dir(dir))
        .chain(built_in())
        .chain(others.iter().map(|dir| from_dir(dir)))
        .find(|loc| !loc.is_empty())
        .unwrap_or_default()
}

/// The [`TABLES`] built into the binary by the `embedded` feature.
#[cfg(feature = "embedded")]
#[must_use]
pub fn built_in() -> Option<Localization> {
    let tables: Vec<(&str, String)> = crate::embedded::TEXT
        .iter()
        .filter(|(table, _)| TABLES.contains(table))
        .filter_map(|(table, deflated)| {
            let json = miniz_oxide::inflate::decompress_to_vec(deflated).ok()?;
            let json = String::from_utf8(json).ok()?;
            Some((*table, json))
        })
        .collect();
    if tables.is_empty() {
        return None;
    }
    let pairs: Vec<(&str, &str)> = tables
        .iter()
        .map(|(table, json)| (*table, json.as_str()))
        .collect();
    Some(Localization::from_json(&pairs))
}

/// No text is built in without the `embedded` feature.
#[cfg(not(feature = "embedded"))]
#[must_use]
pub const fn built_in() -> Option<Localization> {
    None
}

/// Where [`discover`] looks, in order: `$STS2_LOCALIZATION`, the sts2-cli copy in this
/// repository's `referee/vendor/sts2-cli` and `referee/vendor/upstream`, and
/// `./localization_eng`.
#[must_use]
pub fn candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os(ENV_VAR) {
        dirs.push(PathBuf::from(dir));
    }
    for vendor in ["sts2-cli", "upstream"] {
        dirs.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../referee/vendor")
                .join(vendor)
                .join("localization_eng"),
        );
    }
    dirs.push(PathBuf::from("localization_eng"));
    dirs
}

/// Styled terminal lines for `markup` (newlines kept), on top of `base`.
#[must_use]
pub fn lines(markup: &str, base: Role) -> Vec<Line<'static>> {
    sts2_view::text::lines(markup, base)
        .into_iter()
        .map(|rich| Line::from(rich.0.into_iter().map(span).collect::<Vec<_>>()))
        .collect()
}

/// [`lines`] on one line: line breaks become spaces.
#[must_use]
pub fn inline(markup: &str, base: Role) -> Vec<Span<'static>> {
    sts2_view::text::inline(markup, base)
        .0
        .into_iter()
        .map(span)
        .collect()
}

/// `rich` as terminal spans.
#[must_use]
pub fn spans(rich: &Rich) -> Vec<Span<'static>> {
    rich.0.iter().cloned().map(span).collect()
}

fn span(part: RichSpan) -> Span<'static> {
    let mut style = theme::style(part.role);
    if part.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Span::styled(part.text, style)
}
