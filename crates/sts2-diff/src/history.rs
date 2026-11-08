//! `sts2-diff history-predict`: what the engine predicts for a recorded run from its seed and
//! the point types of its path, without playing any fight (`tools/replay_history.py` drives it).
//!
//! A run's save history keeps, per act, the map point types in visit order and the room each
//! one became (`ENCOUNTER.X`, `EVENT.X`, `shop`, ...). The engine's map, room lists and `?`
//! rolls depend only on the seed and on which nodes were visited, so for every act this finds
//! the paths through the generated map with the recorded point types, walks each one the way
//! `Run::enter_point` would (the `?` roll, the next encounter or event) and reports the best
//! match: the first floor where the prediction and the history disagree, and how.

use serde_json::{Value, json};
use sts2_core::content::relics::{RelicData, RelicKind};
use sts2_core::rng::RunRngType;
use sts2_core::run::odds::UnknownMapPointOdds;
use sts2_core::run::rooms::{self, RoomSet};
use sts2_core::run::{
    Act1, ActMap, MapCoord, MapPoint, MapPointType, RoomType, Run, RunCard, RunRelic, RunState,
    UnlockState, hooks,
};
use sts2_core::{Character, Rng};

use crate::Result;

/// More candidate paths than this are not tried (a type sequence matches few paths).
const MAX_PATHS: usize = 256;

/// The history's `map_point_type` of a generated point.
const fn point_name(kind: MapPointType) -> &'static str {
    match kind {
        MapPointType::Unassigned => "unassigned",
        MapPointType::Unknown => "unknown",
        MapPointType::Shop => "shop",
        MapPointType::Treasure => "treasure",
        MapPointType::RestSite => "rest_site",
        MapPointType::Monster => "monster",
        MapPointType::Elite => "elite",
        MapPointType::Boss => "boss",
        MapPointType::Ancient => "ancient",
    }
}

/// The history's `room_type` of a room.
const fn room_name(room: RoomType) -> &'static str {
    match room {
        RoomType::Monster => "monster",
        RoomType::Elite => "elite",
        RoomType::Boss => "boss",
        RoomType::Event => "event",
        RoomType::Shop => "shop",
        RoomType::Treasure => "treasure",
        RoomType::RestSite => "rest_site",
        RoomType::Map => "map",
    }
}

/// One recorded floor: the map point type and the first room's type and model id.
struct Recorded<'a> {
    point: &'a str,
    room: &'a str,
    model: Option<&'a str>,
    /// The relics held on entering (`RELIC.X` ids).
    relics: Vec<&'a str>,
    /// A `LanternKey` was in the deck on entering (its hook makes Glory's `?` rooms events).
    lantern_key: bool,
}

/// A candidate path walked: what the engine would have in each room.
struct Walk {
    /// Per floor (the ancient first): `(room type name, model id)`.
    rooms: Vec<(&'static str, Option<String>)>,
    /// The `?` stream after the walk.
    rng: Rng,
}

/// The paths from the start through points of the recorded types, ending at the boss, and the
/// deepest floor any prefix reached with the child types there (for a failed search).
fn find_paths(
    map: &ActMap,
    recorded: &[Recorded<'_>],
) -> (Vec<Vec<MapCoord>>, (usize, Vec<&'static str>)) {
    fn go(
        map: &ActMap,
        recorded: &[Recorded<'_>],
        at: &MapPoint,
        depth: usize,
        path: &mut Vec<MapCoord>,
        out: &mut Vec<Vec<MapCoord>>,
        deepest: &mut (usize, Vec<&'static str>),
    ) {
        if out.len() >= MAX_PATHS {
            return;
        }
        if depth == recorded.len() {
            out.push(path.clone());
            return;
        }
        let mut children = at.children.to_vec();
        children.sort();
        if depth > deepest.0 {
            *deepest = (
                depth,
                children
                    .iter()
                    .filter_map(|&c| map.point(c))
                    .map(|p| point_name(p.kind))
                    .collect(),
            );
        }
        for child in children {
            let Some(point) = map.point(child) else {
                continue;
            };
            if point_name(point.kind) != recorded[depth].point {
                continue;
            }
            path.push(child);
            go(map, recorded, point, depth + 1, path, out, deepest);
            path.pop();
        }
    }
    let mut out = Vec::new();
    let mut deepest = (0, Vec::new());
    if recorded.first().is_some_and(|r| r.point == "ancient") {
        go(
            map,
            recorded,
            map.start(),
            1,
            &mut vec![map.start().coord],
            &mut out,
            &mut deepest,
        );
    }
    (out, deepest)
}

/// The act a walk is in.
#[derive(Clone, Copy)]
struct ActInfo {
    index: usize,
    /// A `GoldenCompass` was taken in this act (`GoldenPathAct`).
    golden: bool,
    /// The profile's first run (`NumberOfRuns == 0`, which `Run::new` does not support): the
    /// run's first two `?` points are events and the third a monster, with no draw
    /// (`UnknownMapPointOdds.Roll`, `UnknownMapPointOdds.cs:125-136`).
    first_run: bool,
    /// The `?` points of the earlier acts (`MapPointHistory` counts every act).
    unknowns_before: usize,
}

/// Walks `path` as `Run::enter_point` does, from a state whose `?` stream is `rng`.
fn walk(
    state: &RunState,
    recorded: &[Recorded<'_>],
    map: &ActMap,
    rooms: &RoomSet,
    path: &[MapCoord],
    rng: Rng,
    act: ActInfo,
) -> Walk {
    let mut rng = rng;
    let mut odds = UnknownMapPointOdds::default();
    let mut normal = 0;
    let mut elite = 0;
    let mut bosses = 0;
    // The room before an act's first node is the previous boss or the ancient: not a shop.
    let mut after_shop = false;
    let mut out = Walk {
        rooms: vec![("event", Some(rooms.ancient.to_owned()))],
        rng: rng.clone(),
    };
    for &coord in &path[1..] {
        let point = map.point(coord).expect("path points exist");
        let before_shops = !point.children.is_empty()
            && point
                .children
                .iter()
                .all(|c| map.point(c).is_some_and(|p| p.kind == MapPointType::Shop));
        let unknowns = act.unknowns_before
            + recorded[1..out.rooms.len()]
                .iter()
                .filter(|r| r.point == "unknown")
                .count();
        let room = if let Some(room) = RoomType::for_point(point.kind) {
            room
        } else if act.first_run && unknowns < 2 {
            RoomType::Event
        } else if act.first_run && unknowns == 2 {
            RoomType::Monster
        } else {
            let mut types = UnknownMapPointOdds::room_types();
            if after_shop || before_shops {
                types.retain(|&t| t != RoomType::Shop);
            }
            let mut held = state.clone();
            held.act_index = act.index;
            held.relics = recorded[out.rooms.len()]
                .relics
                .iter()
                .filter_map(|id| RelicKind::from_id(id.trim_start_matches("RELIC.")))
                .map(RunRelic::new)
                .collect();
            if recorded[out.rooms.len()].lantern_key {
                held.deck.push(RunCard::new("LANTERN_KEY"));
            }
            if act.golden {
                // `GoldenCompass.AfterObtained` records the act it was taken in.
                for relic in &mut held.relics {
                    if let RelicData::GoldenCompass(data) = &mut relic.data {
                        data.golden_path_act =
                            i32::try_from(act.index).expect("an act index fits i32");
                    }
                }
            }
            hooks::modify_unknown_map_point_room_types(&held, &mut types);
            odds.roll(&types, &mut rng)
        };
        let model = match room {
            RoomType::Monster => {
                let id = rooms.normal_encounters[normal % rooms.normal_encounters.len()].id;
                normal += 1;
                Some(id.to_owned())
            }
            RoomType::Elite => {
                let id = rooms.elite_encounters[elite % rooms.elite_encounters.len()].id;
                elite += 1;
                Some(id.to_owned())
            }
            // `NextBossEncounter`: the second boss (A10, last act) after the first.
            RoomType::Boss => {
                let id = rooms
                    .second_boss
                    .filter(|_| bosses > 0)
                    .unwrap_or(rooms.boss)
                    .id;
                bosses += 1;
                Some(id.to_owned())
            }
            // An event is the next one the pool allows, which depends on the run's gold, HP, deck
            // and relics: only the order is checked, in `first_mismatch`.
            _ => None,
        };
        after_shop = room == RoomType::Shop;
        out.rooms.push((room_name(room), model));
    }
    out.rng = rng;
    out
}

/// How a walk compares with the history: the first disagreeing floor and what differs, if any.
fn first_mismatch(
    walk: &Walk,
    recorded: &[Recorded<'_>],
    events: &[&str],
    types_only: bool,
) -> Option<(usize, Value)> {
    let mut last_event_index: Option<usize> = None;
    for (floor, ((room, model), rec)) in walk.rooms.iter().zip(recorded).enumerate() {
        let recorded_model = rec.model.map(|m| m.split_once('.').map_or(m, |(_, id)| id));
        let mismatch = |kind: &str, predicted: Value, recorded: Value| {
            // Held relics the engine does not port: their `?` odds hooks are missing.
            let unported: Vec<&str> = rec
                .relics
                .iter()
                .copied()
                .filter(|id| RelicKind::from_id(id.trim_start_matches("RELIC.")).is_none())
                .collect();
            Some((
                floor,
                json!({
                    "floor": floor, "kind": kind, "point": rec.point,
                    "predicted": predicted, "recorded": recorded, "unported_relics": unported,
                }),
            ))
        };
        if *room != rec.room {
            return mismatch("room_type", json!(room), json!(rec.room));
        }
        match *room {
            _ if types_only => {}
            "monster" | "elite" | "boss" => {
                if model.as_deref() != recorded_model {
                    return mismatch("encounter", json!(model), json!(recorded_model));
                }
            }
            "event" if floor == 0 => {
                // The act's ancient (Neow in Act 1).
                if model.as_deref() != recorded_model {
                    return mismatch("ancient", json!(model), json!(recorded_model));
                }
            }
            "event" => {
                // The pool is walked in order, skipping events the run can't have; the history's
                // events must follow the list's order.
                let Some(id) = recorded_model else { continue };
                // `WarHistorianRepy` refuses every pull (`IsAllowed` is false); Lantern Key's
                // `ModifyNextEvent` puts it in place of Act 3's next pulled event
                // (`LanternKey.cs:30-37`), whose list index the history does not show.
                if id == "WAR_HISTORIAN_REPY" {
                    continue;
                }
                let Some(index) = events.iter().position(|e| *e == id) else {
                    return mismatch("event_not_in_pool", json!(null), json!(id));
                };
                if last_event_index.is_some_and(|last| index <= last) {
                    return mismatch(
                        "event_order",
                        json!(events[last_event_index.unwrap_or(0)]),
                        json!(id),
                    );
                }
                last_event_index = Some(index);
            }
            _ => {}
        }
    }
    None
}

/// The input's acts as recorded floors.
fn recorded_acts(input: &Value) -> Result<Vec<Vec<Recorded<'_>>>> {
    input["acts"]
        .as_array()
        .ok_or("no acts")?
        .iter()
        .map(|act| {
            Ok(act
                .as_array()
                .ok_or("act is not a list")?
                .iter()
                .map(|e| Recorded {
                    point: e["point"].as_str().unwrap_or(""),
                    room: e["room"].as_str().unwrap_or(""),
                    model: e["model"].as_str(),
                    lantern_key: e["lantern"].as_bool() == Some(true),
                    relics: e["relics"]
                        .as_array()
                        .map(|r| r.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default(),
                })
                .collect())
        })
        .collect()
}

/// Predicts a recorded run.
///
/// `input` is `{"seed", "ascension", "character", "act1", "spoils": [bool per act], "types_only", "acts": [[{"point",
/// "room", "model", "relics", "lantern"}, ...], ...]}`: the acts of `map_point_history` reduced to each
/// point's type, first room and the relics held on entering. `spoils[a]` is whether a `SpoilsMap`
/// was in the deck when act `a` began; `golden_act` is the act a `GoldenCompass` was taken in; `act1` (`"overgrowth"`, the default, or `"underdocks"`) is the
/// recorded Act 1, which the engine is forced to; `types_only` skips the encounter and event checks.
/// `first_run` walks the `?` points as on a profile's first run (no draw for the first three).
/// `character` is a class name (default `Necrobinder`). With `"run"` (the `.run` file's JSON) the
/// result also has `replay`, the floor-by-floor checks of [`crate::history_replay::replay`].
/// `act1_roll` is the engine's own roll ([`rooms::select_acts`] with [`Act1::Random`]): 0 Overgrowth,
/// 1 Underdocks.
///
/// # Errors
/// If the input is malformed or the run cannot be created.
pub fn predict(input: &Value) -> Result<Value> {
    let seed = input["seed"].as_str().ok_or("no seed")?;
    let ascension = u8::try_from(input["ascension"].as_u64().ok_or("no ascension")?)?;
    let act1 = match input["act1"].as_str() {
        None => Act1::Overgrowth,
        Some(key) => Act1::from_key(key).ok_or_else(|| format!("unknown act1 {key}"))?,
    };
    let character = match input["character"].as_str() {
        None => Character::Necrobinder,
        Some(name) => {
            Character::from_name(name).ok_or_else(|| format!("unknown character {name}"))?
        }
    };
    let run = Run::new(character, seed, ascension, UnlockState::all(), act1)?;
    let state = run.state();
    let rolled = rooms::select_acts(state.rng.seed(), &UnlockState::all(), Act1::Random)[0];
    let act_roll = u8::from(rolled.id == rooms::UNDERDOCKS.id);
    let types_only = input["types_only"].as_bool() == Some(true);
    let first_run = input["first_run"].as_bool() == Some(true);
    let mut unknowns_before = 0;

    // The `?` stream is the run's, so each act continues where the last one ended.
    let mut rng = state.rng[RunRngType::UnknownMapPoint].clone();
    let mut per_act = Vec::new();
    for (index, recorded) in recorded_acts(input)?.iter().enumerate() {
        let before = unknowns_before;
        unknowns_before += recorded.iter().filter(|r| r.point == "unknown").count();
        if index >= state.acts.len() {
            per_act.push(json!({"act": index, "status": "unsupported", "why": "no such act"}));
            continue;
        }
        let spoils = input["spoils"][index].as_bool() == Some(true);
        // The act a Golden Compass was taken in (`GoldenPathAct`): its map is the golden path.
        let golden = input["golden_act"].as_u64() == Some(index as u64);
        let generated = if golden {
            Ok(ActMap::golden_path())
        } else if spoils {
            ActMap::generate_spoils(&state.rng, index, ascension)
        } else {
            ActMap::generate(&state.rng, index, ascension)
        };
        let map = match generated {
            Ok(map) => map,
            Err(e) => {
                per_act.push(json!({"act": index, "status": "map_error", "why": e.to_string()}));
                continue;
            }
        };
        let rooms = &state.acts[index];
        let (paths, deepest) = find_paths(&map, recorded);
        if paths.is_empty() {
            per_act.push(json!({"act": index, "status": "mismatch", "paths": 0,
                "mismatch": {"kind": "no_map_path", "deepest_floor": deepest.0,
                    "recorded": recorded.get(deepest.0).map(|r| r.point), "available": deepest.1}}));
            continue;
        }
        // The path that agrees longest (all of it, if one does).
        let mut best: Option<(Walk, Option<(usize, Value)>)> = None;
        for path in &paths {
            let walked = walk(
                state,
                recorded,
                &map,
                rooms,
                path,
                rng.clone(),
                ActInfo {
                    index,
                    golden,
                    first_run,
                    unknowns_before: before,
                },
            );
            let mismatch = first_mismatch(&walked, recorded, &rooms.events, types_only);
            let reached = |m: &Option<(usize, Value)>| m.as_ref().map_or(usize::MAX, |&(f, _)| f);
            if best
                .as_ref()
                .is_none_or(|(_, b)| reached(&mismatch) > reached(b))
            {
                best = Some((walked, mismatch));
            }
        }
        let Some((walked, mismatch)) = best else {
            continue;
        };
        rng = walked.rng;
        let mut entry = json!({"act": index, "status": "match", "paths": paths.len(),
            "floors": recorded.len()});
        if let Some((_, m)) = mismatch {
            entry["status"] = json!("mismatch");
            entry["mismatch"] = m;
        }
        per_act.push(entry);
    }
    let mut out = json!({"act1_roll": act_roll, "acts": per_act});
    if !input["run"].is_null() {
        out["replay"] =
            crate::history_replay::replay(character, seed, ascension, act1, &input["run"])?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Lantern Key's `WarHistorianRepy` in Act 3 does not count as the list's next pull
    /// (community run `X5WDEJGLZ5J0`: `TRIAL` after it).
    #[test]
    fn war_historian_repy_is_not_in_pull_order() {
        let events = ["TRIAL", "WAR_HISTORIAN_REPY", "SELF_HELP_BOOK"];
        let mut rooms = vec![("event", None); 4];
        rooms[0].1 = Some("NEOW".to_owned());
        let walk = Walk {
            rooms,
            rng: Rng::new(0),
        };
        let rec = |model| Recorded {
            point: "unknown",
            room: "event",
            model: Some(model),
            relics: Vec::new(),
            lantern_key: false,
        };
        let recorded = [
            rec("EVENT.NEOW"),
            rec("EVENT.WAR_HISTORIAN_REPY"),
            rec("EVENT.TRIAL"),
            rec("EVENT.SELF_HELP_BOOK"),
        ];
        assert_eq!(first_mismatch(&walk, &recorded, &events, false), None);
        let out_of_order = [
            rec("EVENT.NEOW"),
            rec("EVENT.SELF_HELP_BOOK"),
            rec("EVENT.TRIAL"),
        ];
        assert!(first_mismatch(&walk, &out_of_order, &events, false).is_some());
    }

    /// A profile's first run (`NumberOfRuns == 0`, PX19: community run `ZHZHRGSTRVEH`): the
    /// first three `?` points are event, event, monster and draw nothing, so the fourth rolls the
    /// stream's first value at base odds. The golden path's fourth `?` is before its shop.
    #[test]
    fn first_run_question_marks_draw_nothing() {
        let seed = "PX19FIRSTRUN";
        let run = Run::new(
            Character::Ironclad,
            seed,
            0,
            UnlockState::all(),
            Act1::Overgrowth,
        )
        .expect("a run");
        let mut rng = run.state().rng[RunRngType::UnknownMapPoint].clone();
        let mut types = UnknownMapPointOdds::room_types();
        types.retain(|&t| t != RoomType::Shop);
        let fourth = room_name(UnknownMapPointOdds::default().roll(&types, &mut rng));
        let rooms = [
            "monster",
            "event",
            "monster",
            "rest_site",
            "monster",
            "rest_site",
            "event",
            "treasure",
            "monster",
            "treasure",
            fourth,
            "shop",
            "elite",
            "rest_site",
            "elite",
            "rest_site",
        ];
        let map = ActMap::golden_path();
        let floor = |point: &str, room: &str| json!({"point": point, "room": room});
        let mut act = vec![floor("ancient", "event")];
        act.extend(
            map.points()
                .iter()
                .zip(rooms)
                .map(|(p, room)| floor(point_name(p.kind), room)),
        );
        act.push(floor("boss", "boss"));
        let input = json!({"seed": seed, "ascension": 0, "character": "Ironclad",
            "golden_act": 0, "types_only": true, "first_run": true, "acts": [act]});
        let out = predict(&input).expect("a prediction");
        assert_eq!(out["acts"][0]["status"], "match", "{out}");
    }
}
