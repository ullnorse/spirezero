//! Start records with the relics' fields and the run's streams (RC1): a record written from a
//! run's fight rebuilds that fight exactly.

use sts2_core::Character;
use sts2_core::run::{Act1, Run, UnlockState};
use sts2_rl::StartRecord;

/// The runs' fights, each with the record written when it started; the run plays the `n`-th
/// legal action, `n` counting up, so it moves on.
fn fights(character: Character, seed: &str, max: usize) -> Vec<(Run, StartRecord)> {
    let mut run = Run::new(character, seed, 0, UnlockState::all(), Act1::default())
        .expect("a run")
        .with_combat_start_streams();
    let mut fights = Vec::new();
    for step in 0..3_000 {
        let in_combat = run.combat().is_some();
        let actions = run.legal_actions();
        let Some(action) = actions.get(step % actions.len().max(1)).cloned() else {
            break;
        };
        if run.apply(action).is_err() {
            continue;
        }
        if !in_combat && let Some(record) = StartRecord::from_run(&run, format!("{seed}/{step}")) {
            fights.push((run.clone(), record));
            if fights.len() == max {
                break;
            }
        }
    }
    fights
}

#[test]
fn a_record_with_streams_and_relic_data_rebuilds_the_fight() {
    let mut checked = 0;
    for (i, character) in Character::ALL.into_iter().enumerate() {
        for (run, record) in fights(character, &format!("RCONE{i}"), 3) {
            let rng = record
                .rng
                .as_ref()
                .expect("the run keeps its fights' streams");
            assert_ne!(rng.seed_string(), record.seed);
            let line = record.to_json();
            let back = StartRecord::from_json(&line).expect("the record reads back");
            assert_eq!(back, record);
            let Some(combat) = back.combat() else {
                continue;
            };
            let fight = run.combat().expect("in the fight");
            if fight.state().room_type != combat.state().room_type {
                // An event's fight: the record's room type is the encounter's list's.
                continue;
            }
            assert_eq!(combat.state(), fight.state(), "{line}");
            checked += 1;
        }
    }
    assert!(checked >= 5, "only {checked} fights checked");
}

#[test]
fn a_record_without_them_still_loads() {
    let (_, record) = fights(Character::Ironclad, "RCONEOLD", 1)
        .pop()
        .expect("a fight");
    let mut old: serde_json::Value = serde_json::from_str(&record.to_json()).expect("json");
    let object = old.as_object_mut().expect("an object");
    object.remove("relic_data");
    object.remove("rng");
    let line = old.to_string();
    let back = StartRecord::from_json(&line).expect("an older record loads");
    assert_eq!((back.relic_data, back.rng), (None, None));
    assert_eq!(
        StartRecord::from_json(&line).expect("again").to_json(),
        line,
        "an older record writes back as it was"
    );
    let mut bad: serde_json::Value = serde_json::from_str(&record.to_json()).expect("json");
    bad["rng"] = "SEED 1 2 3".into();
    assert!(StartRecord::from_json(&bad.to_string()).is_err());
    bad["rng"] = serde_json::Value::from(sts2_core::text::to_text(
        record.rng.as_ref().expect("streams"),
    ));
    assert!(StartRecord::from_json(&bad.to_string()).is_ok());
    bad["relic_data"] = serde_json::json!(["Nunchaku 0 0"]);
    assert!(StartRecord::from_json(&bad.to_string()).is_err());
}
