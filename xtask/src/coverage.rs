//! `cargo xtask coverage [--character C] [--act A | --all] [--verbose] [--allow-missing]`: what of
//! one character's Act 1, Hive or Glory is not ported yet (default Necrobinder, Overgrowth).
//!
//! Inventory entities (`docs/inventory/<character>-<act>.json`) are compared with the content
//! entities, the entity files in `crates/sts2-core/src/content/<kind>/` that the registries are
//! generated from. Entities that are multiplayer-only or listed in `docs/coverage-exclusions.md`
//! are out of scope and not counted. `--all` prints a matrix of every character and act instead.

use crate::json::{self, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

const INVENTORY_DIR: &str = "docs/inventory";
/// The characters with an inventory, in matrix order.
const CHARACTERS: &[&str] = &["Ironclad", "Silent", "Defect", "Regent", "Necrobinder"];
/// The acts with an inventory (Act 1 variants, then Hive and Glory).
pub const ACTS: &[&str] = &["overgrowth", "underdocks", "hive", "glory"];
const EXCLUSIONS: &str = "docs/coverage-exclusions.md";
pub const CONTENT: &str = "crates/sts2-core/src/content";

/// Report kinds, with the content module directory that holds the ported ones.
pub const KINDS: &[(&str, &str)] = &[
    ("card", "cards"),
    ("relic", "relics"),
    ("potion", "potions"),
    ("power", "powers"),
    ("monster", "monsters"),
    ("encounter", "encounters"),
    ("event", "events"),
    ("enchantment", "enchantments"),
    ("affliction", "afflictions"),
];

/// Lines of output without `--verbose`.
const MAX_LINES: usize = 60;
/// Width at which lists of ids wrap.
const WIDTH: usize = 100;

/// Normalize an id for matching: ignore case and `_`, and the `Power` suffix of power ids.
pub fn normalize(kind: &str, id: &str) -> String {
    let id = if kind == "power" {
        id.strip_suffix("Power").unwrap_or(id)
    } else {
        id
    };
    id.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Normalizes a variant name of `entity_variants`, which never carries the `Power` suffix (the
/// variant of `FreePowerPower` is `FreePower`, which must not lose its own `Power`).
pub fn normalize_variant(id: &str) -> String {
    normalize("", id)
}

/// Whether the abstract base class `id` (`TemporaryStrengthPower`) is ported as a module of shared
/// code: it has no enum variant, only `content/<dir>/<snake_case>.rs` (power ids lose `Power`).
fn abstract_module_exists(kind: &str, dir: &str, id: &str) -> bool {
    let id = if kind == "power" {
        id.strip_suffix("Power").unwrap_or(id)
    } else {
        id
    };
    let mut name = String::new();
    for (i, c) in id.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            name.push('_');
        }
        name.extend(c.to_lowercase());
    }
    std::path::Path::new(&format!("{CONTENT}/{dir}/{name}.rs")).exists()
}

/// The variants `sts2-core/build.rs` generates for `content/<dir>`: one per `<snake_case>.rs`
/// file, except `mod.rs`, the shared modules `mod.rs` declares by hand and test-only files
/// (`#![cfg(test)]`).
pub fn entity_variants(dir: &str) -> Result<Vec<String>, String> {
    variants_in(CONTENT, dir)
}

/// [`entity_variants`] under the content directory `root`. The inventory lists ancients (Neow,
/// Darv, ..) as events, so the variants of `events` include those of `ancients`.
fn variants_in(root: &str, dir: &str) -> Result<Vec<String>, String> {
    let mut variants = dir_variants(root, dir)?;
    if dir == "events" {
        variants.extend(dir_variants(root, "ancients")?);
        variants.sort();
        variants.dedup();
    }
    Ok(variants)
}

fn dir_variants(root: &str, dir: &str) -> Result<Vec<String>, String> {
    let path = format!("{root}/{dir}");
    let mod_rs = std::fs::read_to_string(format!("{path}/mod.rs"))
        .map_err(|e| format!("{path}/mod.rs: {e}"))?;
    let shared: BTreeSet<&str> = mod_rs
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = ["pub(crate) ", "pub(super) ", "pub ", ""]
                .into_iter()
                .find_map(|vis| line.strip_prefix(vis)?.strip_prefix("mod "))?;
            rest.strip_suffix(';')
        })
        .collect();
    let mut variants = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| format!("{path}: {e}"))? {
        let file = entry.map_err(|e| format!("{path}: {e}"))?.path();
        let Some(stem) = file
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|_| file.extension().is_some_and(|e| e == "rs"))
        else {
            continue;
        };
        if stem == "mod" || shared.contains(stem) {
            continue;
        }
        let source =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        if source.lines().any(|l| l.trim() == "#![cfg(test)]") {
            continue;
        }
        variants.push(
            stem.split('_')
                .flat_map(|word| {
                    let mut chars = word.chars();
                    chars
                        .next()
                        .map(|c| c.to_ascii_uppercase())
                        .into_iter()
                        .chain(chars)
                })
                .collect(),
        );
    }
    variants.sort();
    Ok(variants)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ported ancient (`content/ancients/`) counts as a ported event, once even if both
    /// directories hold it (`Neow`), and a test-only ancient does not count.
    #[test]
    fn ancients_count_as_ported_events() -> Result<(), String> {
        let root = format!(
            "{}/../target/coverage-test-{}",
            env!("CARGO_MANIFEST_DIR"),
            std::process::id()
        );
        let write = |path: &str, text: &str| -> Result<(), String> {
            let path = format!("{root}/{path}");
            let dir = std::path::Path::new(&path).parent().ok_or("no parent")?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| e.to_string())
        };
        write("events/mod.rs", "")?;
        write("events/big_fish.rs", "")?;
        write("events/neow.rs", "")?;
        write("ancients/mod.rs", "")?;
        write("ancients/neow.rs", "")?;
        write("ancients/darv.rs", "")?;
        write("ancients/test_ancient.rs", "#![cfg(test)]\n")?;
        let variants = variants_in(&root, "events");
        let relics = variants_in(&root, "ancients");
        std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        assert_eq!(variants?, ["BigFish", "Darv", "Neow"]);
        assert_eq!(relics?, ["Darv", "Neow"]);
        Ok(())
    }
}

/// One character and act whose coverage is reported; the act is lower case (`overgrowth`).
#[derive(Clone, Copy)]
struct Scope<'a> {
    character: &'a str,
    act: &'a str,
}

/// An out-of-scope entity: its `kind:Id` key and whether its exclusion is still doubtful.
struct Exclusions {
    keys: BTreeSet<String>,
    check: BTreeSet<String>,
}

/// Does the `@Ironclad,!underdocks` list of an exclusion line apply to `scope`? Names are
/// characters or acts; any positive name of a family must match, no negated name may.
fn applies(list: &str, scope: Scope<'_>) -> bool {
    let (mut chars, mut acts) = (Vec::new(), Vec::new());
    for token in list.split(',') {
        let (negated, name) = token
            .strip_prefix('!')
            .map_or((false, token), |name| (true, name));
        let (family, value) = if ACTS.contains(&name) {
            (&mut acts, scope.act)
        } else {
            (&mut chars, scope.character)
        };
        family.push((negated, name == value));
    }
    [chars, acts].iter().all(|family| {
        let positive = family.iter().filter(|(negated, _)| !negated);
        (positive.clone().next().is_none() || positive.clone().any(|(_, hit)| *hit))
            && !family.iter().any(|(negated, hit)| *negated && *hit)
    })
}

/// Parse `- kind:Id [@scope] reason` lines for `scope`; a reason that starts with `check` marks a
/// doubtful exclusion, and a `@Character,!act` list limits the line to matching scopes.
fn parse_exclusions(text: &str, scope: Scope<'_>) -> Exclusions {
    let mut out = Exclusions {
        keys: BTreeSet::new(),
        check: BTreeSet::new(),
    };
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("- ") else {
            continue;
        };
        let (key, reason) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        if !key.contains(':') {
            continue;
        }
        let mut reason = reason.trim_start_matches(['—', '-', ' ']);
        if let Some(list) = reason.strip_prefix('@') {
            let (list, tail) = list.split_once(char::is_whitespace).unwrap_or((list, ""));
            if !applies(list, scope) {
                continue;
            }
            reason = tail.trim_start();
        }
        if reason.starts_with("check") {
            out.check.insert(key.to_owned());
        }
        out.keys.insert(key.to_owned());
    }
    out
}

/// Counts and missing ids for one kind.
#[derive(Default)]
struct Row {
    ported: usize,
    in_scope: usize,
    missing: Vec<String>,
    /// Unported entities excluded with a doubtful ("check") reason.
    check_unported: usize,
    /// Ported variants that match no inventory entity.
    uninventoried: Vec<String>,
    excluded: usize,
}

/// Wrap `ids` into lines of at most [`WIDTH`] characters after `indent`.
pub fn wrap(ids: &[String], indent: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for id in ids {
        if !line.is_empty() && indent.len() + line.len() + 1 + id.len() > WIDTH {
            lines.push(format!("{indent}{line}"));
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(id);
    }
    if !line.is_empty() {
        lines.push(format!("{indent}{line}"));
    }
    lines
}

/// Compute the report for the inventory `doc`.
fn rows(doc: &Value, exclusions: &Exclusions) -> Result<BTreeMap<&'static str, Row>, String> {
    let entities = doc
        .get("entities")
        .and_then(Value::as_array)
        .ok_or("inventory has no `entities` array")?;
    let mut out = BTreeMap::new();
    for (kind, dir) in KINDS {
        let variants = entity_variants(dir)?;
        let ported: BTreeSet<String> = variants
            .iter()
            .map(String::as_str)
            .map(normalize_variant)
            .collect();
        let mut row = Row::default();
        let mut inventory_ids = BTreeSet::new();
        for entity in entities {
            let field = |name| entity.get(name).and_then(Value::as_str).unwrap_or_default();
            if field("kind") != *kind {
                continue;
            }
            let id = field("id");
            inventory_ids.insert(normalize(kind, id));
            let key = format!("{kind}:{id}");
            let multiplayer = matches!(entity.get("multiplayer_only"), Some(Value::Bool(true)));
            if multiplayer || exclusions.keys.contains(&key) {
                row.excluded += 1;
                if exclusions.check.contains(&key) && !ported.contains(&normalize(kind, id)) {
                    row.check_unported += 1;
                }
                continue;
            }
            row.in_scope += 1;
            let is_abstract = matches!(
                entity.get("attrs").and_then(|a| a.get("abstract")),
                Some(Value::Bool(true))
            );
            if ported.contains(&normalize(kind, id))
                || (is_abstract && abstract_module_exists(kind, dir, id))
            {
                row.ported += 1;
            } else {
                row.missing.push(id.to_owned());
            }
        }
        row.uninventoried = variants
            .into_iter()
            .filter(|v| !inventory_ids.contains(&normalize_variant(v)))
            .collect();
        out.insert(*kind, row);
    }
    Ok(out)
}

/// The parsed command line.
struct Args {
    verbose: bool,
    allow_missing: bool,
    all: bool,
    character: String,
    act: String,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    const USAGE: &str = "usage: cargo xtask coverage [--character C] [--act overgrowth|underdocks|hive|glory | --all] [--verbose] [--allow-missing]";
    let mut out = Args {
        verbose: false,
        allow_missing: false,
        all: false,
        character: "Necrobinder".to_owned(),
        act: "overgrowth".to_owned(),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--verbose" => out.verbose = true,
            "--allow-missing" => out.allow_missing = true,
            "--all" => out.all = true,
            "--character" | "--act" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("`{arg}` needs a value\n{USAGE}"))?;
                if arg == "--act" {
                    out.act = value.to_lowercase();
                } else {
                    out.character.clone_from(value);
                }
            }
            bad => return Err(format!("unknown argument `{bad}`\n{USAGE}")),
        }
    }
    if !out.all {
        if !CHARACTERS.contains(&out.character.as_str()) {
            return Err(format!("unknown character `{}`\n{USAGE}", out.character));
        }
        if !ACTS.contains(&out.act.as_str()) {
            return Err(format!("unknown act `{}`\n{USAGE}", out.act));
        }
    }
    Ok(out)
}

/// The report rows of one character and act.
fn report(scope: Scope<'_>) -> Result<BTreeMap<&'static str, Row>, String> {
    let inventory = format!(
        "{INVENTORY_DIR}/{}-{}.json",
        scope.character.to_lowercase(),
        scope.act
    );
    let text = std::fs::read_to_string(&inventory)
        .map_err(|e| format!("{inventory}: {e} (run from the repo root)"))?;
    let doc = json::parse(&text)?;
    let exclusions = std::fs::read_to_string(EXCLUSIONS)
        .map_err(|e| format!("{EXCLUSIONS}: {e}"))
        .map(|t| parse_exclusions(&t, scope))?;
    rows(&doc, &exclusions)
}

/// The acts whose inventories (any character's) list each entity, keyed `kind:normalized id`.
pub fn inventory_acts() -> Result<BTreeMap<String, BTreeSet<&'static str>>, String> {
    let mut out: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
    for character in CHARACTERS {
        for act in ACTS {
            let inventory = format!("{INVENTORY_DIR}/{}-{act}.json", character.to_lowercase());
            let text = std::fs::read_to_string(&inventory)
                .map_err(|e| format!("{inventory}: {e} (run from the repo root)"))?;
            let doc = json::parse(&text)?;
            let entities = doc
                .get("entities")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{inventory} has no `entities` array"))?;
            for entity in entities {
                let field = |name| entity.get(name).and_then(Value::as_str).unwrap_or_default();
                let kind = field("kind");
                out.entry(format!("{kind}:{}", normalize(kind, field("id"))))
                    .or_default()
                    .insert(act);
            }
        }
    }
    Ok(out)
}

/// The `--all` matrix: ported/in scope per character (rows) and act (columns), with the missing count.
fn matrix(allow_missing: bool) -> Result<(Vec<String>, bool), String> {
    let mut header = format!("{:<12}", "character");
    for act in ACTS {
        write!(header, " {act:>24}").map_err(|e| e.to_string())?;
    }
    let mut lines = vec![header];
    let mut any_missing = false;
    for character in CHARACTERS {
        let mut line = format!("{character:<12}");
        for act in ACTS {
            let rows = report(Scope { character, act })?;
            let ported: usize = rows.values().map(|r| r.ported).sum();
            let scope: usize = rows.values().map(|r| r.in_scope).sum();
            let missing: usize = rows.values().map(|r| r.missing.len()).sum();
            any_missing |= missing > 0;
            let cell = format!("{ported}/{scope} ({missing} missing)");
            write!(line, " {cell:>24}").map_err(|e| e.to_string())?;
        }
        lines.push(line);
    }
    Ok((lines, any_missing && !allow_missing))
}

/// Run the subcommand; returns the lines to print and whether anything in scope is missing.
pub fn run(args: &[String]) -> Result<(Vec<String>, bool), String> {
    let Args {
        verbose,
        allow_missing,
        all,
        character,
        act,
    } = parse_args(args)?;
    if all {
        return matrix(allow_missing);
    }
    let rows = report(Scope {
        character: &character,
        act: &act,
    })?;

    let mut lines = vec![format!(
        "{:<12} {:>7} {:>9} {:>8} {:>9}",
        "kind", "ported", "in scope", "missing", "excluded"
    )];
    let (mut ported, mut scope, mut missing, mut excluded) = (0, 0, 0, 0);
    for (kind, row) in &rows {
        lines.push(format!(
            "{kind:<12} {:>7} {:>9} {:>8} {:>9}",
            row.ported,
            row.in_scope,
            row.missing.len(),
            row.excluded
        ));
        ported += row.ported;
        scope += row.in_scope;
        missing += row.missing.len();
        excluded += row.excluded;
    }
    lines.push(format!(
        "{:<12} {ported:>7} {scope:>9} {missing:>8} {excluded:>9}",
        "total"
    ));
    let check: usize = rows.values().map(|r| r.check_unported).sum();
    if check > 0 {
        lines.push(format!(
            "note: {check} excluded entities are unported and marked \"check\" in {EXCLUSIONS}"
        ));
    }

    let extra: Vec<String> = rows
        .iter()
        .flat_map(|(kind, r)| r.uninventoried.iter().map(move |id| format!("{kind}:{id}")))
        .collect();
    if !extra.is_empty() {
        lines.push(format!(
            "note: {} ported variants are not in the inventory (not counted)",
            extra.len()
        ));
        if verbose {
            lines.extend(wrap(&extra, "  "));
        }
    }

    let with_missing: Vec<_> = rows.iter().filter(|(_, r)| !r.missing.is_empty()).collect();
    if !with_missing.is_empty() {
        lines.push(String::new());
        lines.push("missing:".to_owned());
    }
    let budget = MAX_LINES
        .saturating_sub(lines.len())
        .checked_div(with_missing.len().max(1))
        .unwrap_or(0);
    for (kind, row) in with_missing {
        let wrapped = wrap(&row.missing, "  ");
        lines.push(format!("{kind} ({}):", row.missing.len()));
        // Each kind gets an equal share of the line budget; its header line is part of it.
        let room = budget.saturating_sub(1).max(1);
        if verbose || wrapped.len() <= room {
            lines.extend(wrapped);
        } else {
            lines.extend(wrapped.iter().take(room.saturating_sub(1)).cloned());
            lines.push("  ... more (use --verbose)".to_owned());
        }
    }
    Ok((lines, missing > 0 && !allow_missing))
}
