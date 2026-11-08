//! `run-gen` traces that pin harness behaviour the referee shows in run mode.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// A scratch directory of this checkout's target (`CARGO_TARGET_TMPDIR`), not the machine-wide
/// `/tmp`: two worktrees testing at once would overwrite each other's files there.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// The trace of `run-gen` on `seed` with the greedy policy (policy seed `index`), `max_actions` long.
fn greedy_trace(seed: &str, index: u32, max_actions: u32) -> Vec<Value> {
    let dir = scratch(&format!("run-gen-{seed}-{max_actions}"));
    let (actions, trace) = (dir.join("actions.json"), dir.join("trace.jsonl"));
    let status = Command::new(env!("CARGO_BIN_EXE_sts2-diff"))
        .args([
            "run-gen",
            "--seed",
            seed,
            "--policy-seed",
            &index.to_string(),
        ])
        .args([
            "--max-actions",
            &max_actions.to_string(),
            "--policy",
            "greedy",
        ])
        .arg("--actions")
        .arg(&actions)
        .arg("--trace")
        .arg(&trace)
        .output()
        .expect("run-gen runs");
    assert!(status.status.success(), "run-gen failed");
    std::fs::read_to_string(trace)
        .expect("trace")
        .lines()
        .map(|l| serde_json::from_str(l).expect("trace line"))
        .collect()
}

/// A `Wellspring` Bottle on a full belt: the potion does not fit, the player leaves it and the
/// event finishes (`RewardsSet.Offer` skips the unselected reward; `referee/tracer/Rewards.cs`).
/// `RL2S74` step 72 used to keep the event's page up because the referee's reward set faulted.
#[test]
fn wellspring_bottle_on_a_full_belt_skips_the_potion_and_finishes() {
    let trace = greedy_trace("RL2S74", 74, 73);
    let line = &trace[72];
    assert_eq!(line["action"]["action"], "choose_option");
    assert_eq!(line["decision"], "map_select");
    assert_eq!(line["run"]["room"], "MapRoom");
    assert_eq!(line["run"]["pending"]["kind"], "map");
    // The belt was full before the Bottle and the potion was skipped, not swapped in.
    assert_eq!(line["run"]["potions"], trace[71]["run"]["potions"]);
    assert_eq!(trace[73]["action"]["action"], "select_map_node");
    assert_eq!(trace[73]["decision"], "rest_site");
}

/// The same Bottle with a free slot takes the potion and leaves the event (`RL2S51` step 27).
#[test]
fn wellspring_bottle_with_a_free_slot_finishes() {
    let trace = greedy_trace("RL2S51", 51, 28);
    let line = &trace[27];
    assert_eq!(line["action"]["action"], "choose_option");
    assert_eq!(line["decision"], "map_select");
    assert_eq!(line["run"]["pending"]["kind"], "map");
    assert_eq!(line["run"]["potions"][0], "FLEX_POTION");
}

/// A run loadout is handed over once the run first waits on the map, after Neow: the cards join the
/// deck, the gold is set and a relic is obtained (`Pear`'s `AfterObtained` raises max HP by 10),
/// and the referee gets the loadout as a `run_loadout` action.
#[test]
fn a_run_loadout_is_handed_over_after_neow() {
    let dir = scratch("run-loadout");
    let (loadout, actions, trace) = (
        dir.join("loadout.json"),
        dir.join("actions.json"),
        dir.join("trace.jsonl"),
    );
    std::fs::write(
        &loadout,
        r#"{"add_cards": ["STRIKE_NECROBINDER+1"], "gold": 321, "add_relics": ["PEAR"]}"#,
    )
    .expect("loadout");
    let output = Command::new(env!("CARGO_BIN_EXE_sts2-diff"))
        .args(["run-gen", "--seed", "RL2S0", "--policy-seed", "0"])
        .args(["--max-actions", "2", "--policy", "greedy"])
        .arg("--run-loadout")
        .arg(&loadout)
        .arg("--actions")
        .arg(&actions)
        .arg("--trace")
        .arg(&trace)
        .output()
        .expect("run-gen runs");
    assert!(output.status.success(), "run-gen failed");
    let lines: Vec<Value> = std::fs::read_to_string(trace)
        .expect("trace")
        .lines()
        .map(|l| serde_json::from_str(l).expect("trace line"))
        .collect();
    let (neow, after) = (&lines[1]["run"], &lines[2]);
    assert_eq!(after["action"]["action"], "run_loadout");
    assert_eq!(after["run"]["gold"], 321);
    assert_eq!(
        after["run"]["deck"].as_array().map(Vec::len),
        Some(neow["deck"].as_array().map_or(0, Vec::len) + 1)
    );
    assert_eq!(
        after["run"]["relics"].as_array().and_then(|r| r.last()),
        Some(&Value::from("PEAR"))
    );
    assert_eq!(
        after["run"]["max_hp"].as_i64(),
        neow["max_hp"].as_i64().map(|hp| hp + 10)
    );
}

/// A hits file also records the effects that ran, not just the entities that appeared: the
/// combat event log gives played cards, used potions and changed powers (`tools/hits.sh --effects`).
#[test]
fn run_gen_writes_effect_hits() {
    let dir = scratch("run-gen-effects");
    let (actions, trace, hits) = (
        dir.join("actions.json"),
        dir.join("trace.jsonl"),
        dir.join("hits.json"),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_sts2-diff"))
        .args(["run-gen", "--seed", "RL2S0", "--policy-seed", "0"])
        .args(["--max-actions", "200", "--policy", "greedy"])
        .arg("--hits")
        .arg(&hits)
        .arg("--actions")
        .arg(&actions)
        .arg("--trace")
        .arg(&trace)
        .output()
        .expect("run-gen runs");
    assert!(output.status.success(), "run-gen failed");
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(hits).expect("hits")).expect("hits json");
    let cards = doc["effects"]["card"].as_array().expect("played cards");
    assert!(!cards.is_empty(), "a 200-action run plays cards");
    assert!(
        doc["card"].as_array().is_some_and(|hits| !hits.is_empty()),
        "the presence hits are still written"
    );
}
