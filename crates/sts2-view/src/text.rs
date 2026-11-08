//! The game's English text and the little template language its strings are written in.
//!
//! None of that text lives in this repository: a client loads the localization JSON (the TUI's
//! `text` module does, the iOS app bundles it) and hands the tables to [`Localization`]. This
//! module is pure: no files, no terminal, no environment.
//!
//! A string is plain text with `[gold]..[/gold]`-style markup and `{Name:formatter:args}`
//! placeholders (`SmartFormat`). [`format`] fills the placeholders from a lookup of named values;
//! [`inline`] and [`lines`] turn the markup into [`Role`]-tagged [`Rich`] text. A renderer maps
//! the roles to its own styles.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The localization tables the TUI reads, one JSON file each.
pub const TABLES: [&str; 17] = [
    "cards",
    "afflictions",
    "card_keywords",
    "combat_messages",
    "static_hover_tips",
    "card_selection",
    "gameplay_ui",
    "relics",
    "potions",
    "powers",
    "events",
    "ascension",
    "characters",
    "acts",
    "enchantments",
    "orbs",
    "modifiers",
];

/// The tables only the phone client loads on top of [`TABLES`]: the game's wording for the rest
/// site, the ancients and the monsters' intents.
pub const WEB_TABLES: [&str; 3] = ["rest_site_ui", "ancients", "intents"];

/// What a piece of text means. A renderer maps the role to its palette; the role never carries a
/// terminal colour, so the same text works in the TUI, on a phone and in a light theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// No styling.
    #[default]
    Plain,
    /// The game's body text.
    Body,
    /// An enemy attack intent, attack damage.
    Attack,
    /// Block, a Defend intent.
    Defend,
    /// Buffs on anyone, an enemy's buff intent.
    Buff,
    /// Debuffs on anyone, debuff intents, status cards.
    Debuff,
    /// A hidden or unknown intent, stunned, asleep.
    Unknown,
    /// You die if nothing changes, or this play kills.
    Lethal,
    /// HP: the bars and the counts.
    Hp,
    /// Healing.
    Heal,
    /// Energy, costs.
    Energy,
    /// Gold.
    Gold,
    /// The selected card, the pickable targets.
    Selected,
    /// Hints, rules, dead enemies, what cannot be played.
    Muted,
    /// The keys that pick something; bold in the default colour.
    Key,
    /// Section titles and names.
    Title,
    /// Bold text in the default colour.
    Emphasis,
    /// What an upgrade changes on a card: the game's green.
    Upgrade,
}

/// One run of text with one meaning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichSpan {
    pub role: Role,
    /// The game's `[b]` tag; a renderer adds bold on top of the role.
    pub bold: bool,
    pub text: String,
    /// An animation tag around the text with its attributes (`rainbow freq=0.3`, `scramble`):
    /// the renderer plays it where it can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx: Option<String>,
}

/// Text with [`Role`]s: the input of every client's renderer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rich(pub Vec<RichSpan>);

impl Rich {
    /// The text without its roles.
    #[must_use]
    pub fn text(&self) -> String {
        self.0.iter().map(|span| span.text.as_str()).collect()
    }

    /// Whether there is nothing to draw.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|span| span.text.is_empty())
    }
}

/// The tables' `KEY.field` -> text maps.
#[derive(Clone, Debug, Default)]
pub struct Localization {
    tables: HashMap<String, HashMap<String, String>>,
}

impl Localization {
    /// No text at all: every lookup misses.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// A localization from `(table, json)` pairs (for tests and embedders).
    #[must_use]
    pub fn from_json(tables: &[(&str, &str)]) -> Self {
        let mut loc = Self::empty();
        for (table, json) in tables {
            loc.add_table(table, json);
        }
        loc
    }

    fn add_table(&mut self, table: &str, json: &str) {
        if let Ok(entries) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.tables.insert(table.to_owned(), entries);
        }
    }

    /// The keys of `table` (none if it is not loaded).
    pub fn keys(&self, table: &str) -> impl Iterator<Item = &str> {
        self.tables
            .get(table)
            .into_iter()
            .flat_map(|t| t.keys().map(String::as_str))
    }

    /// Whether no table was loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// The text at `key` (`ENTRY.field`) of `table`.
    #[must_use]
    pub fn get(&self, table: &str, key: &str) -> Option<&str> {
        self.tables.get(table)?.get(key).map(String::as_str)
    }
}

/// `n` hundredths as a decimal without trailing zeros (`125` is `1.25`).
fn hundredths(n: i64) -> String {
    let sign = if n < 0 { "-" } else { "" };
    let (whole, frac) = (n.abs() / 100, n.abs() % 100);
    let frac = format!("{frac:02}");
    format!("{sign}{whole}.{}", frac.trim_end_matches('0'))
}

/// A value a placeholder can take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    /// A card var's shown value and the value `diff()` compares it with
    /// (`DynamicVar.ToHighlightedString`: `PreviewValue` against `EnchantedValue`).
    Compared {
        value: i64,
        base: i64,
    },
    /// A non-integral decimal, in hundredths (`1.25` is 125).
    Hundredths(i64),
    Flag(bool),
    /// A name, compared by `choose(Name)`.
    Name(String),
}

impl Value {
    const fn truthy(&self) -> bool {
        match self {
            Self::Int(n) | Self::Hundredths(n) | Self::Compared { value: n, .. } => *n != 0,
            Self::Flag(b) => *b,
            Self::Name(_) => true,
        }
    }
}

/// How an unknown value is shown.
fn unknown(name: &str) -> String {
    format!("[dim]‹{name}›[/dim]")
}

/// Fills the `{...}` placeholders of `template`. `vars` gives the value of a name; a name it
/// does not know is shown as `‹Name›`.
#[must_use]
pub fn format(template: &str, vars: &dyn Fn(&str) -> Option<Value>) -> String {
    let mut out = String::new();
    expand(&mut out, template, vars, None);
    out
}

fn expand(
    out: &mut String,
    template: &str,
    vars: &dyn Fn(&str) -> Option<Value>,
    cur: Option<&Value>,
) {
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = matching_brace(after) else {
            out.push_str(&rest[open..]);
            return;
        };
        placeholder(out, &after[..close], vars, cur);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
}

/// The index of the `}` that closes a `{` that was just consumed.
fn matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => return Some(i),
            '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// `s` split at its top-level `|`s.
fn forms(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

fn icons(n: i64, icon: char) -> String {
    let count = usize::try_from(n).unwrap_or(0);
    if count <= 3 {
        std::iter::repeat_n(icon, count).collect()
    } else {
        format!("{count}{icon}")
    }
}

/// The text of `(arg)` after a formatter name, if `s` starts with `name(`.
fn call_arg<'a>(s: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let args = s.strip_prefix(name)?.strip_prefix('(')?;
    let close = args.find(')')?;
    Some((&args[..close], &args[close + 1..]))
}

fn placeholder(
    out: &mut String,
    inner: &str,
    vars: &dyn Fn(&str) -> Option<Value>,
    cur: Option<&Value>,
) {
    let (name, spec) = inner
        .split_once(':')
        .map_or((inner, None), |(n, s)| (n, Some(s)));
    let name = name.trim();
    let value = if name.is_empty() {
        cur.cloned()
    } else {
        vars(name)
    };
    let shown = |out: &mut String, v: Option<&Value>| match v {
        Some(Value::Int(n) | Value::Compared { value: n, .. }) => out.push_str(&n.to_string()),
        Some(Value::Hundredths(n)) => out.push_str(&hundredths(*n)),
        Some(Value::Name(s)) => out.push_str(s),
        Some(Value::Flag(b)) => out.push_str(&b.to_string()),
        None => out.push_str(&unknown(name)),
    };
    let Some(spec) = spec else {
        shown(out, value.as_ref());
        return;
    };
    let int = |v: &Option<Value>| match v {
        Some(Value::Int(n) | Value::Compared { value: n, .. }) => Some(*n),
        _ => None,
    };
    match spec {
        // `PercentMoreFormatter`: `(x - 1) * 100`; an int is already the percentage.
        "percentMore()" if matches!(value, Some(Value::Hundredths(_))) => {
            if let Some(Value::Hundredths(n)) = value {
                out.push_str(&(n - 100).to_string());
            }
        }
        // `PercentLessFormatter`: `(1 - x) * 100`.
        "percentLess()" if matches!(value, Some(Value::Hundredths(_))) => {
            if let Some(Value::Hundredths(n)) = value {
                out.push_str(&(100 - n).to_string());
            }
        }
        // `StsTextUtilities.HighlightChangeText`: green when the value is up on its base, red
        // when down (the other way round for `inverseDiff`).
        "diff()" | "inverseDiff()" if matches!(value, Some(Value::Compared { .. })) => {
            if let Some(Value::Compared { value, base }) = value {
                highlighted(out, value, base, spec == "inverseDiff()");
            }
        }
        "diff()" | "inverseDiff()" | "percentMore()" | "percentLess()" => {
            shown(out, value.as_ref());
        }
        "abs()" => match int(&value) {
            Some(n) => out.push_str(&n.abs().to_string()),
            None => shown(out, None),
        },
        "energyIcons()" | "starIcons()" => match int(&value) {
            Some(n) => out.push_str(&icons(n, if spec == "starIcons()" { '★' } else { '◆' })),
            None => shown(out, None),
        },
        _ => {
            if let Some((n, _)) = call_arg(spec, "energyIcons") {
                out.push_str(&icons(n.trim().parse().unwrap_or(1), '◆'));
            } else if let Some(body) = spec.strip_prefix("plural:") {
                let parts = forms(body);
                let pick = match int(&value) {
                    Some(1) => parts.first(),
                    _ => parts.get(1).or_else(|| parts.first()),
                };
                expand(out, pick.copied().unwrap_or_default(), vars, value.as_ref());
            } else if let Some(body) = spec.strip_prefix("show:") {
                pick_branch(
                    out,
                    &forms(body),
                    value.as_ref().is_some_and(Value::truthy),
                    vars,
                    value.as_ref(),
                );
            } else if let Some((arg, body)) = call_arg(spec, "choose") {
                let body = body.strip_prefix(':').unwrap_or(body);
                let hit = matches!(&value, Some(Value::Name(s)) if s == arg);
                pick_branch(out, &forms(body), hit, vars, value.as_ref());
            } else if let Some(body) = spec.strip_prefix("cond:") {
                conditional(out, body, value.as_ref(), vars);
            } else {
                // `{Flag:then|else}`.
                pick_branch(
                    out,
                    &forms(spec),
                    value.as_ref().is_some_and(Value::truthy),
                    vars,
                    value.as_ref(),
                );
            }
        }
    }
}

/// `StsTextUtilities.HighlightChangeText`: `value` green when it is up on `base`, red when
/// down (the other way round when `inverse`).
fn highlighted(out: &mut String, value: i64, base: i64, inverse: bool) {
    let order = if inverse {
        base.cmp(&value)
    } else {
        value.cmp(&base)
    };
    let color = match order {
        std::cmp::Ordering::Greater => "green",
        std::cmp::Ordering::Less => "red",
        std::cmp::Ordering::Equal => {
            out.push_str(&value.to_string());
            return;
        }
    };
    out.push_str(&["[", color, "]", &value.to_string(), "[/", color, "]"].concat());
}

/// `{Neg:cond:<0?A|B}` compares; `{Name.StringValue:cond:A|B}` (no comparison) takes A when the
/// value is set, else B.
fn conditional(
    out: &mut String,
    body: &str,
    value: Option<&Value>,
    vars: &dyn Fn(&str) -> Option<Value>,
) {
    let Some(((cmp, k), branches)) = condition(body) else {
        pick_branch(
            out,
            &forms(body),
            value.is_some_and(Value::truthy),
            vars,
            value,
        );
        return;
    };
    let hit = match value {
        Some(Value::Int(n) | Value::Compared { value: n, .. }) => match cmp {
            '<' => *n < k,
            '>' => *n > k,
            _ => *n == k,
        },
        _ => false,
    };
    pick_branch(out, &forms(branches), hit, vars, value);
}

/// `<0?A|B` -> ((`<`, 0), `A|B`); `None` for a branch list with no comparison before its `?`.
fn condition(body: &str) -> Option<((char, i64), &str)> {
    let (test, branches) = body.split_once('?')?;
    let mut chars = test.chars();
    let cmp = chars
        .next()
        .filter(|c| matches!(c, '<' | '>' | '=' | '!'))?;
    Some(((cmp, chars.as_str().trim().parse().ok()?), branches))
}

fn pick_branch(
    out: &mut String,
    parts: &[&str],
    first: bool,
    vars: &dyn Fn(&str) -> Option<Value>,
    cur: Option<&Value>,
) {
    let pick = if first { parts.first() } else { parts.get(1) };
    expand(out, pick.copied().unwrap_or_default(), vars, cur);
}

/// How a tag changes the current tone: a role, bold, or nothing visible (an animation tag still
/// nests, so its closing tag pops).
enum Tag {
    Color(Role),
    Bold,
    /// An animation tag: no look here, the span carries it (`RichSpan::fx`).
    Fx,
}

fn tag(tag: &str) -> Option<Tag> {
    Some(match tag {
        "gold" => Tag::Color(Role::Gold),
        "blue" | "aqua" => Tag::Color(Role::Defend),
        "red" | "orange" => Tag::Color(Role::Attack),
        "green" => Tag::Color(Role::Buff),
        "purple" => Tag::Color(Role::Debuff),
        "b" => Tag::Bold,
        "dim" => Tag::Color(Role::Muted),
        "sine" | "jitter" | "shake" | "rainbow" | "scramble" => Tag::Fx,
        _ => return None,
    })
}

#[derive(Clone, Copy)]
struct Tone<'a> {
    role: Role,
    bold: bool,
    /// The innermost open animation tag as written, attributes and all.
    fx: Option<&'a str>,
}

/// Rich text for `markup` (newlines kept as line breaks), on top of `base`.
#[must_use]
pub fn lines(markup: &str, base: Role) -> Vec<Rich> {
    let mut lines = vec![Rich::default()];
    let mut stack = vec![Tone {
        role: base,
        bold: false,
        fx: None,
    }];
    let mut rest = markup;
    let mut text = String::new();
    let flush = |text: &mut String, stack: &[Tone<'_>], lines: &mut Vec<Rich>| {
        let tone = stack.last().copied().unwrap_or(Tone {
            role: base,
            bold: false,
            fx: None,
        });
        for (i, piece) in std::mem::take(text).split('\n').enumerate() {
            if i > 0 {
                lines.push(Rich::default());
            }
            if !piece.is_empty()
                && let Some(line) = lines.last_mut()
            {
                line.0.push(RichSpan {
                    role: tone.role,
                    bold: tone.bold,
                    text: piece.to_owned(),
                    fx: tone.fx.map(str::to_owned),
                });
            }
        }
    };
    while let Some(open) = rest.find('[') {
        text.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let found = after.find(']').map(|close| (&after[..close], close));
        let known = found.and_then(|(t, close)| {
            let (closing, name) = t.strip_prefix('/').map_or((false, t), |n| (true, n));
            // A tag's attributes (`[rainbow freq=0.3]`) do not change which tag it is.
            tag(name.split(' ').next().unwrap_or(name)).map(|tag| (closing, tag, close, name))
        });
        if let Some((closing, tag, close, name)) = known {
            flush(&mut text, &stack, &mut lines);
            if closing {
                if stack.len() > 1 {
                    stack.pop();
                }
            } else {
                let top = stack.last().copied().unwrap_or(Tone {
                    role: base,
                    bold: false,
                    fx: None,
                });
                stack.push(match tag {
                    Tag::Color(role) => Tone { role, ..top },
                    Tag::Bold => Tone { bold: true, ..top },
                    Tag::Fx => Tone {
                        fx: Some(name),
                        ..top
                    },
                });
            }
            rest = &after[close + 1..];
        } else {
            text.push('[');
            rest = after;
        }
    }
    text.push_str(rest);
    flush(&mut text, &stack, &mut lines);
    lines
}

/// [`lines`] on one line: line breaks become spaces.
#[must_use]
pub fn inline(markup: &str, base: Role) -> Rich {
    let mut out = Rich::default();
    for (i, line) in lines(markup, base).into_iter().enumerate() {
        if i > 0 {
            out.0.push(RichSpan {
                fx: None,
                role: base,
                bold: false,
                text: " ".to_owned(),
            });
        }
        out.0.extend(line.0);
    }
    out
}

/// `effect` with the numbers that differ from `base` lit: raised ones (Strength) as buffs,
/// lowered ones (Weak) as debuffs. `live` is the plain text `effect` was built from; the words
/// are matched by position.
#[must_use]
pub fn lit_changes(effect: Rich, base: &str, live: &str) -> Rich {
    let number = |word: &str| {
        word.trim_matches(|c: char| !c.is_ascii_digit())
            .parse::<i64>()
            .ok()
            .filter(|_| word.chars().any(|c| c.is_ascii_digit()))
    };
    let changed: Vec<(usize, Role)> = base
        .split_whitespace()
        .zip(live.split_whitespace())
        .enumerate()
        .filter_map(|(i, (b, l))| match (number(b), number(l)) {
            (Some(b), Some(l)) if l > b => Some((i, Role::Buff)),
            (Some(b), Some(l)) if l < b => Some((i, Role::Debuff)),
            _ => None,
        })
        .collect();
    if changed.is_empty() {
        return effect;
    }
    let mut out = Vec::new();
    let mut words = 0;
    let mut mid_word = false;
    for span in effect.0 {
        let pieces: Vec<&str> = span.text.split(' ').collect();
        let last = pieces.len() - 1;
        for (i, piece) in pieces.iter().enumerate() {
            if !piece.is_empty() {
                if !mid_word {
                    words += 1;
                    mid_word = true;
                }
                let lit = changed
                    .iter()
                    .find(|(w, _)| *w + 1 == words)
                    .map(|(_, r)| *r);
                out.push(lit.map_or_else(
                    || RichSpan {
                        fx: None,
                        text: (*piece).to_owned(),
                        ..span.clone()
                    },
                    |role| RichSpan {
                        fx: None,
                        role,
                        bold: true,
                        text: (*piece).to_owned(),
                    },
                ));
            }
            if i < last {
                out.push(RichSpan {
                    fx: None,
                    text: " ".to_owned(),
                    ..span.clone()
                });
                mid_word = false;
            }
        }
    }
    Rich(out)
}

/// `after` with its words that are not in `before` (outside their longest common run of words)
/// marked [`Role::Upgrade`]: what an upgrade changes on a card.
#[must_use]
pub fn mark_changes(after: Rich, before: &str) -> Rich {
    let old: Vec<&str> = before.split_whitespace().collect();
    let plain = after.text();
    let new: Vec<&str> = plain.split_whitespace().collect();
    // Longest common subsequence, by word.
    let mut common = vec![vec![0_usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            common[i][j] = if old[i] == new[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let mut kept = vec![false; new.len()];
    let (mut i, mut j) = (0, 0);
    while i < old.len() && j < new.len() {
        if old[i] == new[j] {
            kept[j] = true;
            i += 1;
            j += 1;
        } else if common[i + 1][j] >= common[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let mut out = Vec::new();
    let mut words = 0;
    let mut mid_word = false;
    for span in after.0 {
        let mut rest = span.text.as_str();
        while let Some(first) = rest.chars().next() {
            let space = first.is_whitespace();
            let end = rest
                .find(|c: char| c.is_whitespace() != space)
                .unwrap_or(rest.len());
            let (piece, tail) = rest.split_at(end);
            rest = tail;
            if space {
                mid_word = false;
            } else if !mid_word {
                words += 1;
                mid_word = true;
            }
            let lit = !space && !kept.get(words - 1).copied().unwrap_or(true);
            out.push(RichSpan {
                fx: None,
                role: if lit { Role::Upgrade } else { span.role },
                bold: span.bold,
                text: piece.to_owned(),
            });
        }
    }
    Rich(out)
}

/// `markup` without its tags (newlines kept).
#[must_use]
pub fn plain(markup: &str) -> String {
    lines(markup, Role::Plain)
        .iter()
        .map(Rich::text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(name: &str) -> Option<Value> {
        Some(match name {
            "Damage" => Value::Int(6),
            "One" => Value::Int(1),
            "Up" => Value::Flag(true),
            "Down" => Value::Flag(false),
            "TargetType" => Value::Name("AllEnemies".to_owned()),
            "Neg" => Value::Int(-2),
            "Gold" => Value::Hundredths(125),
            "Half" => Value::Hundredths(50),
            _ => return None,
        })
    }

    fn run(template: &str) -> String {
        plain(&format(template, &vars))
    }

    #[test]
    fn numbers_and_unknowns() {
        assert_eq!(run("Deal {Damage:diff()} damage."), "Deal 6 damage.");
        assert_eq!(run("Deal {Missing:diff()}."), "Deal ‹Missing›.");
        assert_eq!(run("{Neg:abs()}"), "2");
        assert_eq!(run("no braces"), "no braces");
        assert_eq!(run("open { only"), "open { only");
    }

    #[test]
    fn diff_colours_a_value_against_its_base() {
        let compared = |value, base| {
            move |name: &str| (name == "D").then_some(Value::Compared { value, base })
        };
        assert_eq!(format("{D:diff()}", &compared(9, 6)), "[green]9[/green]");
        assert_eq!(format("{D:diff()}", &compared(4, 6)), "[red]4[/red]");
        assert_eq!(format("{D:diff()}", &compared(6, 6)), "6");
        assert_eq!(format("{D:inverseDiff()}", &compared(9, 6)), "[red]9[/red]");
        assert_eq!(format("{D} {D:plural:hit|hits}", &compared(1, 2)), "1 hit");
    }

    #[test]
    fn plural_show_choose_and_conditions() {
        assert_eq!(run("{One} {One:plural:card|cards}"), "1 card");
        assert_eq!(run("{Damage} {Damage:plural:card|cards}"), "6 cards");
        assert_eq!(run("{Missing:plural:time|times}"), "times");
        assert_eq!(run("{Damage:plural:|x{}}"), "x6");
        assert_eq!(run("{Up:show:ALL cards|a card}"), "ALL cards");
        assert_eq!(run("{Down:show:ALL cards|a card}"), "a card");
        assert_eq!(run("{Down:show:+1}."), ".");
        assert_eq!(run("{Up:show:{Damage} copies|a copy}"), "6 copies");
        assert_eq!(run("hit{TargetType:choose(AllEnemies): all|}!"), "hit all!");
        assert_eq!(run("{Neg:cond:<0?Decreases|Increases}"), "Decreases");
        // A branch list with no comparison (Archaic Tooth): the first when the value is set.
        let tooth = "Transform {Missing.StringValue:cond:{Damage} into it|a starter card}.";
        assert_eq!(run(tooth), "Transform a starter card.");
        assert_eq!(run("{Up:cond:{Damage} cards|none}"), "6 cards");
        assert_eq!(run("{Down:\n(Hits)|}x"), "x");
        assert_eq!(run("{Up:A|B}"), "A");
        assert_eq!(run("{Missing:A|B}"), "B");
    }

    #[test]
    fn icons_render_counts() {
        assert_eq!(run("{Damage:energyIcons()}"), "6◆");
        assert_eq!(run("{Gold:percentMore()} {Gold}"), "25 1.25");
        assert_eq!(run("{Half:percentLess()} {Half}"), "50 0.5");
        assert_eq!(run("{One:energyIcons()}"), "◆");
        assert_eq!(run("{x:energyIcons(2)}"), "◆◆");
        assert_eq!(run("{One:starIcons()}"), "★");
    }

    #[test]
    fn markup_becomes_rich_lines() {
        let l = lines("a [gold]b[/gold] c\nd [sine]e[/sine] [x]", Role::Plain);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].0[1].role, Role::Gold);
        assert!(!l[0].0[1].bold);
        assert_eq!(l[0].text(), "a b c");
        assert_eq!(plain("d [sine]e[/sine] [x]"), "d e [x]");
        assert_eq!(plain("a [rainbow freq=0.3 sat=0.8]b[/rainbow]"), "a b");
        let rich = lines("[rainbow freq=0.3]b[/rainbow] c", Role::Plain);
        assert_eq!(rich[0].0[0].fx.as_deref(), Some("rainbow freq=0.3"));
        assert_eq!(rich[0].0[1].fx, None);
        let one = inline("a\nb", Role::Plain);
        assert_eq!(one.text(), "a b");
        let bold = lines("[gold][b]x[/b][/gold]", Role::Plain);
        assert_eq!(bold[0].0[0].role, Role::Gold);
        assert!(bold[0].0[0].bold);
    }

    #[test]
    fn localization_loads_tables_and_skips_bad_files() {
        let loc = Localization::from_json(&[
            ("cards", r#"{"A.title": "Alpha"}"#),
            ("relics", "not json"),
        ]);
        assert_eq!(loc.get("cards", "A.title"), Some("Alpha"));
        assert_eq!(loc.get("cards", "B.title"), None);
        assert_eq!(loc.get("relics", "A.title"), None);
        assert!(Localization::empty().is_empty());
    }

    #[test]
    fn an_upgrade_marks_only_what_it_changes() {
        let after = Rich(vec![
            RichSpan {
                fx: None,
                role: Role::Body,
                bold: false,
                text: "Deal ".to_owned(),
            },
            RichSpan {
                fx: None,
                role: Role::Attack,
                bold: true,
                text: "9".to_owned(),
            },
            RichSpan {
                fx: None,
                role: Role::Body,
                bold: false,
                text: " damage.\nDraw 1 card.".to_owned(),
            },
        ]);
        let marked = mark_changes(after.clone(), "Deal 6 damage.");
        assert_eq!(marked.text(), after.text());
        let lit: Vec<&str> = marked
            .0
            .iter()
            .filter(|s| s.role == Role::Upgrade)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(lit, ["9", "Draw", "1", "card."]);
        assert!(marked.0.iter().any(|s| s.text == "9" && s.bold));
    }
}
