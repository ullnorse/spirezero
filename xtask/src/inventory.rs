//! `cargo xtask inventory [--character C] [--act A] [--deps] <id>...`: look entities up in
//! `docs/inventory/<character>-<act>.json` (default Necrobinder, overgrowth).

use crate::json::{self, Value};
use std::collections::HashSet;

/// Does `entity` match `query`, either a bare id (`Reap`) or `kind:Id` (`power:DoomPower`)?
fn matches(entity: &Value, query: &str) -> bool {
    let id = entity.get("id").and_then(Value::as_str);
    match query.split_once(':') {
        Some((kind, wanted)) => {
            id == Some(wanted) && entity.get("kind").and_then(Value::as_str) == Some(kind)
        }
        None => id == Some(query),
    }
}

/// The `kind:Id` key of `entity`.
fn key(entity: &Value) -> String {
    let field = |name| entity.get(name).and_then(Value::as_str).unwrap_or_default();
    format!("{}:{}", field("kind"), field("id"))
}

/// Entities matching `queries`; with `deps`, their transitive dependencies instead.
fn select<'a>(entities: &'a [Value], queries: &[String], deps: bool) -> Vec<&'a Value> {
    let roots: Vec<&Value> = entities
        .iter()
        .filter(|e| queries.iter().any(|q| matches(e, q)))
        .collect();
    if !deps {
        return roots;
    }
    let mut seen: HashSet<String> = roots.iter().map(|e| key(e)).collect();
    let mut pending: Vec<String> = roots.iter().flat_map(|e| dep_keys(e)).rev().collect();
    let mut out = Vec::new();
    while let Some(wanted) = pending.pop() {
        if !seen.insert(wanted.clone()) {
            continue;
        }
        for entity in entities.iter().filter(|e| matches(e, &wanted)) {
            out.push(entity);
            pending.extend(dep_keys(entity).into_iter().rev());
        }
    }
    out
}

fn dep_keys(entity: &Value) -> Vec<String> {
    entity
        .get("deps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| d.as_str().map(str::to_owned))
        .collect()
}

/// Run the subcommand with its arguments; returns the lines to print.
pub fn run(args: &[String]) -> Result<Vec<String>, String> {
    const USAGE: &str =
        "usage: cargo xtask inventory [--character C] [--act A] [--deps] <id|kind:id>...";
    let (mut deps, mut character, mut act) =
        (false, "necrobinder".to_owned(), "overgrowth".to_owned());
    let mut queries = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--deps" => deps = true,
            "--character" | "--act" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("`{arg}` needs a value\n{USAGE}"))?
                    .to_lowercase();
                if arg == "--act" {
                    act = value;
                } else {
                    character = value;
                }
            }
            _ => queries.push(arg.clone()),
        }
    }
    if queries.is_empty() {
        return Err(USAGE.to_owned());
    }
    let inventory = format!("docs/inventory/{character}-{act}.json");
    let text = std::fs::read_to_string(&inventory)
        .map_err(|e| format!("{inventory}: {e} (run from the repo root)"))?;
    let doc = json::parse(&text)?;
    let entities = doc
        .get("entities")
        .and_then(Value::as_array)
        .ok_or("inventory has no `entities` array")?;
    if select(entities, &queries, false).is_empty() {
        return Err(format!("no inventory entry for: {}", queries.join(" ")));
    }
    let found = select(entities, &queries, deps);
    Ok(found.into_iter().map(Value::to_compact).collect())
}
