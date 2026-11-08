//! `cargo xtask hits [--min N] [--summary] [--check ALLOWLIST] [dir...]`: ported entities that no
//! generated run touched.
//!
//! Reads every `*.hits.json` under the directories (default `target/regress/work` and
//! `target/diff`; ids in the referee's form, plus the `character` and `act` the run played) and
//! lists the variants of the content enums in `crates/sts2-core/src/content/*/mod.rs` that appear
//! in none of them. Only a seed that matched the referee has one: `sts2-diff gen`/`run-gen`
//! write `<seed>.hits.pending.json`, which `compare-batch` (`tools/diff.sh`) or `tools/diff_run.sh`
//! renames once the seed's verdict is a match. The default directories keep older runs' files;
//! `tools/hits_sweep.sh` passes a directory it emptied first. A directory that does not exist is
//! an error.
//!
//! - `--min N`: also list the entities hit by fewer than `N` runs (a run is one `*.hits.json`, one
//!   seed), with their counts, and for each character the cards of its own pool hit by fewer than
//!   `N` runs played as that character.
//! - `--summary`: the runs and distinct entities hit per kind, per character and per act.
//! - `--effects`: entities whose effect never ran in any run: a card never played, a potion never
//!   used, a power never applied or changed. Presence alone (the normal report) is not evidence
//!   that the behaviour was exercised.
//! - `--check FILE`: exit non-zero if a ported entity has no hit and is not in `FILE`
//!   (`docs/hits-allowlist.md`: `- kind:Id reason` lines, as `docs/coverage-exclusions.md`; `kind:Id@hive,glory`
//!   excuses the entity only in a check whose `--acts` include one of those acts, and is not reported
//!   stale in any other).
//! - `--acts A,B`: the acts the runs played (`overgrowth,underdocks`). An entity with no hit that
//!   only the other acts' inventories list cannot be reached; it is listed as advisory and does
//!   not fail `--check`.

use crate::coverage::{
    ACTS, KINDS, entity_variants, inventory_acts, normalize, normalize_variant, wrap,
};
use crate::json::{self, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The five characters, with the name of their card pool in the inventory.
const POOLS: &[(&str, &str)] = &[
    ("Ironclad", "IroncladCardPool"),
    ("Silent", "SilentCardPool"),
    ("Defect", "DefectCardPool"),
    ("Regent", "RegentCardPool"),
    ("Necrobinder", "NecrobinderCardPool"),
];
const DEFAULT_DIRS: &[&str] = &["target/regress/work", "target/diff"];

/// Every `*.hits.json` below `dir`, depth first.
fn find(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            find(&path, out);
        } else if path.to_string_lossy().ends_with(".hits.json") {
            out.push(path);
        }
    }
}

/// Runs that touched each normalized entity, per kind; and the same per character, plus the
/// number of runs per character and per act.
#[derive(Default)]
struct Counts {
    runs: usize,
    by_kind: BTreeMap<String, BTreeMap<String, usize>>,
    by_character: BTreeMap<String, BTreeMap<String, BTreeMap<String, usize>>>,
    runs_of: BTreeMap<String, usize>,
    by_act: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// Entities whose effect ran (played card, used potion, changed power) in at least one run.
    effects: BTreeMap<String, BTreeSet<String>>,
}

fn read(files: &[std::path::PathBuf]) -> Result<Counts, String> {
    let mut counts = Counts::default();
    for file in files {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let doc = json::parse(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        let text_of = |key| doc.get(key).and_then(Value::as_str).map(str::to_owned);
        let character = text_of("character").unwrap_or_else(|| "unknown".to_owned());
        let act = text_of("act").unwrap_or_else(|| "forced fight".to_owned());
        counts.runs += 1;
        *counts.runs_of.entry(character.clone()).or_default() += 1;
        if let Some(Value::Obj(fields)) = doc.get("effects") {
            for (kind, ids) in fields {
                for id in ids
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    let id = id
                        .strip_suffix("_POWER")
                        .filter(|_| kind == "power")
                        .unwrap_or(id);
                    counts
                        .effects
                        .entry(kind.clone())
                        .or_default()
                        .insert(normalize(kind, id));
                }
            }
        }
        for (kind, _) in KINDS {
            let ids: BTreeSet<String> = doc
                .get(kind)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                // `DOOM_POWER` is the referee's id of the `Doom` variant.
                .map(|id| {
                    let id = id
                        .strip_suffix("_POWER")
                        .filter(|_| *kind == "power")
                        .unwrap_or(id);
                    normalize(kind, id)
                })
                .collect();
            for id in &ids {
                *counts
                    .by_kind
                    .entry((*kind).to_owned())
                    .or_default()
                    .entry(id.clone())
                    .or_default() += 1;
                *counts
                    .by_character
                    .entry(character.clone())
                    .or_default()
                    .entry((*kind).to_owned())
                    .or_default()
                    .entry(id.clone())
                    .or_default() += 1;
            }
            counts
                .by_act
                .entry(act.clone())
                .or_default()
                .entry((*kind).to_owned())
                .or_default()
                .extend(ids);
        }
    }
    Ok(counts)
}

/// The cards of each character's own pool (normalized ids), from the inventories.
fn pool_cards() -> Result<BTreeMap<&'static str, BTreeSet<String>>, String> {
    let mut pools: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let dir = "docs/inventory";
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))? {
        let path = entry.map_err(|e| format!("{dir}: {e}"))?.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let doc = json::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        for entity in doc
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if entity.get("kind").and_then(Value::as_str) != Some("card") {
                continue;
            }
            let id = entity.get("id").and_then(Value::as_str).unwrap_or_default();
            for pool in entity
                .get("pool")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some((character, _)) = POOLS.iter().find(|(_, p)| pool.as_str() == Some(p)) {
                    pools
                        .entry(character)
                        .or_default()
                        .insert(normalize("card", id));
                }
            }
        }
    }
    Ok(pools)
}

/// An allowlist: `kind:Id` keys to the acts they are scoped to (empty: every check) and their reasons.
type Allowlist = BTreeMap<String, (Vec<String>, String)>;

/// `kind:Id` keys of an allowlist (`- kind:Id[@act,act] reason` lines), with their acts and reasons.
fn read_allowlist(path: &str) -> Result<Allowlist, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("- ") else {
            continue;
        };
        let (key, reason) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        if !key.contains(':') {
            continue;
        }
        if reason.trim().is_empty() {
            return Err(format!("{path}: `{key}` has no reason"));
        }
        let (key, acts) = key
            .split_once('@')
            .map_or((key, Vec::new()), |(key, acts)| {
                (key, acts.split(',').map(str::to_owned).collect())
            });
        if let Some(bad) = acts.iter().find(|a| !ACTS.contains(&a.as_str())) {
            return Err(format!(
                "{path}: `{key}` is scoped to the unknown act {bad}"
            ));
        }
        out.insert(key.to_owned(), (acts, reason.trim().to_owned()));
    }
    Ok(out)
}

/// The options of the command line and its directories.
#[derive(Default)]
struct Options {
    min: usize,
    summary: bool,
    allowlist: Option<String>,
    /// `--acts`: the acts played; empty for every act.
    acts: Vec<String>,
    /// `--effects`: report entities whose effect never ran (card played, potion used, power
    /// changed), not just entities that appeared.
    effects: bool,
    dirs: Vec<String>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--min" => {
                options.min = args
                    .next()
                    .and_then(|n| n.parse().ok())
                    .ok_or("--min needs a number")?;
            }
            "--summary" => options.summary = true,
            "--effects" => options.effects = true,
            "--check" => {
                options.allowlist = Some(args.next().ok_or("--check needs a file")?.clone());
            }
            "--acts" => {
                let acts = args.next().ok_or("--acts needs a list")?;
                options.acts = acts.split(',').map(str::to_owned).collect();
                if let Some(bad) = options.acts.iter().find(|a| !ACTS.contains(&a.as_str())) {
                    return Err(format!(
                        "--acts: unknown act {bad} (one of {})",
                        ACTS.join(",")
                    ));
                }
            }
            dir if !dir.starts_with("--") => options.dirs.push(dir.to_owned()),
            other => return Err(format!("unknown option {other}")),
        }
    }
    if options.dirs.is_empty() {
        options.dirs = DEFAULT_DIRS.iter().map(|d| (*d).to_owned()).collect();
    }
    Ok(options)
}

/// The `--effects` section: how many of each kind's variants ever ran their effect, and the
/// ones that never did. Separate from the presence report because it needs its own tallies.
fn effects_report(counts: &Counts, options: &Options) -> Result<Vec<String>, String> {
    if !options.effects {
        return Ok(Vec::new());
    }
    let mut lines = Vec::new();
    if counts.effects.is_empty() {
        lines.push("effects: no hits file has effect data (regenerate the runs)".to_owned());
        return Ok(lines);
    }
    for kind in ["card", "potion", "power"] {
        let Some((_, dir)) = KINDS.iter().find(|(k, _)| *k == kind) else {
            continue;
        };
        let variants = entity_variants(dir)?;
        let used = counts.effects.get(kind);
        let never: Vec<String> = variants
            .iter()
            .filter(|v| !used.is_some_and(|u| u.contains(&normalize_variant(v))))
            .cloned()
            .collect();
        lines.push(format!(
            "{kind} effects: {}/{} used",
            variants.len() - never.len(),
            variants.len()
        ));
        if !never.is_empty() {
            lines.extend(wrap(&never, "  "));
        }
    }
    Ok(lines)
}

/// Run the subcommand; returns the lines to print and whether `--check` failed.
pub fn run(args: &[String]) -> Result<(Vec<String>, bool), String> {
    let options = parse_args(args)?;
    let mut files = Vec::new();
    for dir in &options.dirs {
        if !Path::new(dir).is_dir() {
            return Err(format!("{dir} is not a directory"));
        }
        find(Path::new(dir), &mut files);
    }
    if files.is_empty() {
        return Err(format!("no *.hits.json under {}", options.dirs.join(" ")));
    }
    let counts = read(&files)?;
    let allowlist = options
        .allowlist
        .as_deref()
        .map(read_allowlist)
        .transpose()?
        .unwrap_or_default();
    let mut lines = vec![format!("{} runs", counts.runs)];
    let mut failed = false;
    let mut zero_keys = BTreeSet::new();
    let acts_of = if options.acts.is_empty() {
        BTreeMap::new()
    } else {
        inventory_acts()?
    };
    for (kind, dir) in KINDS {
        let variants = entity_variants(dir)?;
        let empty = BTreeMap::new();
        let seen = counts.by_kind.get(*kind).unwrap_or(&empty);
        let (allowed, zero): (Vec<String>, Vec<String>) = variants
            .iter()
            .filter(|v| !seen.contains_key(&normalize_variant(v)))
            .cloned()
            .partition(|v| applies(&allowlist, &format!("{kind}:{v}"), &options.acts));
        // Listed by some inventory, but by none of the acts played.
        let (unreachable, unexpected): (Vec<String>, Vec<String>) = zero
            .into_iter()
            .partition(|v| is_unreachable(&acts_of, kind, v, &options.acts));
        zero_keys.extend(allowed.iter().map(|v| format!("{kind}:{v}")));
        lines.push(format!(
            "{kind}: {}/{} hit, {} with 0 hits{}",
            variants.len() - unexpected.len() - unreachable.len() - allowed.len(),
            variants.len(),
            unexpected.len() + unreachable.len() + allowed.len(),
            if allowed.is_empty() {
                String::new()
            } else {
                format!(" ({} allowlisted)", allowed.len())
            },
        ));
        lines.extend(wrap(&unexpected, "  "));
        if !unreachable.is_empty() {
            lines.push(format!(
                "{kind}: {} with 0 hits only in acts not played (advisory)",
                unreachable.len()
            ));
            lines.extend(wrap(&unreachable, "  "));
        }
        failed |= options.allowlist.is_some() && !unexpected.is_empty();
        if options.min > 0 {
            let mut thin: Vec<(usize, String)> = variants
                .iter()
                .filter_map(|v| {
                    let n = seen.get(&normalize_variant(v)).copied()?;
                    (n < options.min).then(|| (n, v.clone()))
                })
                .collect();
            thin.sort();
            let thin: Vec<String> = thin.into_iter().map(|(n, v)| format!("{v}({n})")).collect();
            lines.push(format!(
                "{kind}: {} hit by fewer than {} runs",
                thin.len(),
                options.min
            ));
            lines.extend(wrap(&thin, "  "));
        }
    }
    lines.extend(effects_report(&counts, &options)?);
    let stale: Vec<&str> = allowlist
        .keys()
        .filter(|key| applies(&allowlist, key, &options.acts) && !zero_keys.contains(*key))
        .map(String::as_str)
        .collect();
    if !stale.is_empty() {
        lines.push(format!(
            "allowlist entries that are hit now or not ported (delete them): {}",
            stale.join(" ")
        ));
    }
    if options.min > 0 {
        lines.extend(per_character(&counts, options.min)?);
    }
    if options.summary {
        lines.extend(summary(&counts)?);
    }
    if failed {
        lines.push("hits: ported entities with no hit that are not in the allowlist".to_owned());
    }
    Ok((lines, failed))
}

/// Whether the allowlist excuses `key` (`kind:Id`) in a check of the acts `played` (every act if empty).
fn applies(allowlist: &Allowlist, key: &str, played: &[String]) -> bool {
    allowlist.get(key).is_some_and(|(acts, _)| {
        acts.is_empty() || played.is_empty() || acts.iter().any(|a| played.contains(a))
    })
}

/// Whether `kind` variant `v` is listed by some inventory but by none of the acts `played` (an
/// entity no inventory lists, such as a test monster, stays reachable).
fn is_unreachable(
    acts_of: &BTreeMap<String, BTreeSet<&'static str>>,
    kind: &str,
    v: &str,
    played: &[String],
) -> bool {
    acts_of
        .get(&format!("{kind}:{}", normalize_variant(v)))
        .is_some_and(|acts| !acts.iter().any(|a| played.iter().any(|p| p == a)))
}

/// Per character: the cards of its own pool hit by fewer than `min` runs played as it.
fn per_character(counts: &Counts, min: usize) -> Result<Vec<String>, String> {
    let pools = pool_cards()?;
    let ported: BTreeMap<String, String> = entity_variants("cards")?
        .into_iter()
        .map(|v| (normalize_variant(&v), v))
        .collect();
    let mut lines = Vec::new();
    for (character, _) in POOLS {
        let hits = counts
            .by_character
            .get(*character)
            .and_then(|kinds| kinds.get("card"));
        let mut thin: Vec<(usize, String)> = pools
            .get(character)
            .into_iter()
            .flatten()
            .filter_map(|id| {
                let name = ported.get(id)?;
                let n = hits.and_then(|h| h.get(id)).copied().unwrap_or(0);
                (n < min).then(|| (n, name.clone()))
            })
            .collect();
        thin.sort();
        let total = pools.get(character).map_or(0, |p| {
            p.iter().filter(|id| ported.contains_key(*id)).count()
        });
        lines.push(format!(
            "{character} pool: {} of {total} ported cards hit by fewer than {min} {character} runs ({} runs)",
            thin.len(),
            counts.runs_of.get(*character).copied().unwrap_or(0)
        ));
        let thin: Vec<String> = thin.into_iter().map(|(n, v)| format!("{v}({n})")).collect();
        lines.extend(wrap(&thin, "  "));
    }
    Ok(lines)
}

/// Runs and distinct entities hit per kind, per character and per act.
fn summary(counts: &Counts) -> Result<Vec<String>, String> {
    let kinds: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
    let mut lines = vec![format!(
        "summary: distinct entities hit ({})",
        kinds.join("/")
    )];
    let mut ported = Vec::new();
    for (_, dir) in KINDS {
        ported.push(entity_variants(dir)?.len());
    }
    lines.push(format!(
        "  ported: {}",
        ported
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/")
    ));
    for (character, kinds_hit) in &counts.by_character {
        let hit: Vec<String> = KINDS
            .iter()
            .map(|(k, _)| kinds_hit.get(*k).map_or(0, BTreeMap::len).to_string())
            .collect();
        lines.push(format!(
            "  {character}: {} runs, {}",
            counts.runs_of.get(character).copied().unwrap_or(0),
            hit.join("/")
        ));
    }
    for (act, kinds_hit) in &counts.by_act {
        let hit: Vec<String> = KINDS
            .iter()
            .map(|(k, _)| kinds_hit.get(*k).map_or(0, BTreeSet::len).to_string())
            .collect();
        lines.push(format!("  {act}: {}", hit.join("/")));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glory's Dampen cannot be hit by Act 1 runs; Act 1's Vulnerable and an uninventoried
    /// entity still must be.
    #[test]
    fn zero_hits_only_later_acts_list_are_advisory() {
        let acts_of = BTreeMap::from([
            ("power:dampen".to_owned(), BTreeSet::from(["glory"])),
            (
                "power:vulnerable".to_owned(),
                BTreeSet::from(["overgrowth", "glory"]),
            ),
        ]);
        let act1 = ["overgrowth".to_owned(), "underdocks".to_owned()];
        assert!(is_unreachable(&acts_of, "power", "Dampen", &act1));
        assert!(!is_unreachable(&acts_of, "power", "Vulnerable", &act1));
        assert!(!is_unreachable(&acts_of, "monster", "TestDummy", &act1));
        assert!(!is_unreachable(
            &acts_of,
            "power",
            "Dampen",
            &["glory".to_owned()]
        ));
    }

    /// `kind:Id@act` excuses the entity only in a check that plays that act.
    #[test]
    fn scoped_allowlist_entries_apply_to_their_acts_only() {
        let allowlist = Allowlist::from([
            (
                "monster:Vantom".to_owned(),
                (vec!["hive".to_owned()], "r".to_owned()),
            ),
            ("relic:WingCharm".to_owned(), (Vec::new(), "r".to_owned())),
        ]);
        let played = |acts: &[&str]| acts.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        assert!(applies(&allowlist, "monster:Vantom", &played(&["hive"])));
        assert!(applies(&allowlist, "monster:Vantom", &played(&[])));
        assert!(!applies(&allowlist, "monster:Vantom", &played(&["glory"])));
        assert!(applies(&allowlist, "relic:WingCharm", &played(&["glory"])));
        assert!(!applies(&allowlist, "relic:Other", &played(&["glory"])));
    }
}
