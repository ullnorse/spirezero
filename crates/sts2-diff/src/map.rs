//! Act maps in the referee's `map` JSON shape (`referee/README.md`, run mode, step 0).

use serde_json::{Value, json};
use sts2_core::rng::{RunRngSet, canonicalize_seed};
use sts2_core::run::{ActMap, MapCoord, MapPoint};

use crate::compare::diff;
use crate::{Error, Result};

fn coord_json(coord: MapCoord) -> Value {
    json!([coord.col, coord.row])
}

fn point_json(point: &MapPoint) -> Value {
    // The referee sorts children, so the dump does not expose the game's `HashSet` order.
    let mut children = point.children.to_vec();
    children.sort();
    json!({
        "col": point.coord.col,
        "row": point.coord.row,
        "type": point.kind.name(),
        "children": children.into_iter().map(coord_json).collect::<Vec<_>>(),
    })
}

/// The map as the referee prints it: `nodes` by row then column, `start`, `boss`, `second_boss`.
#[must_use]
pub fn to_json(map: &ActMap) -> Value {
    json!({
        "nodes": map.points().iter().map(point_json).collect::<Vec<_>>(),
        "start": point_json(map.start()),
        "boss": point_json(map.boss()),
        "second_boss": map.second_boss().map_or(Value::Null, point_json),
    })
}

/// Generates the Act 1 map of `seed` (any spelling; it is canonicalised like the game's).
///
/// # Errors
/// If the game would throw while generating this seed's map.
pub fn generate(seed: &str) -> Result<Value> {
    generate_act(seed, 0, 0)
}

/// The map of act `act_index` (0 to 2) at `ascension` in the same shape.
///
/// # Errors
/// If the game would throw while generating this seed's map.
pub fn generate_act(seed: &str, act_index: usize, ascension: u8) -> Result<Value> {
    let rngs = RunRngSet::new(&canonicalize_seed(seed));
    Ok(to_json(
        &ActMap::generate(&rngs, act_index, ascension).map_err(Error::from)?,
    ))
}

/// The first difference between `rust` and the `run.map` of the referee's step-0 trace line.
///
/// # Errors
/// If the line has no map.
pub fn first_divergence(rust: &Value, referee_line: &Value) -> Result<Option<String>> {
    let referee = referee_line
        .pointer("/run/map")
        .ok_or("referee step 0 has no run.map")?;
    Ok(diff(rust, referee, &mut String::new())
        .map(|(path, rust, referee)| format!("map{path}: rust {rust} vs referee {referee}")))
}

/// The first difference between `rust` and `run.act_maps[act_index]` of the referee's step-0
/// line (an actions file with `"act_maps": true`).
///
/// # Errors
/// If the line has no such map.
pub fn first_act_divergence(
    rust: &Value,
    referee_line: &Value,
    act_index: usize,
) -> Result<Option<String>> {
    let referee = referee_line
        .pointer(&format!("/run/act_maps/{act_index}"))
        .ok_or("referee step 0 has no run.act_maps (set \"act_maps\": true)")?;
    Ok(
        diff(rust, referee, &mut String::new()).map(|(path, rust, referee)| {
            format!("act_maps[{act_index}]{path}: rust {rust} vs referee {referee}")
        }),
    )
}
