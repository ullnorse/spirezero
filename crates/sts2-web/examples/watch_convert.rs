//! Converts a run of the RL env (an eval record's `actions`: base64 of little-endian `u16` indices
//! of the run action space) into the web player's actions, for `www/watch/*.json`.
//!
//! `cargo run -p sts2-web --example watch_convert -- EVAL.jsonl SEED TITLE OUT.json`
//!
//! The env plays seed `s` as the game seed string `RUN<s>` ([`sts2_rl::RunConfig::run`]), so the
//! web run is `Game::new(character, "RUN<s>", ascension, [])`. The env also takes single-option
//! decisions by itself, which the web player shows as taps: the env records every engine action it
//! applies, and each is looked up in the web run's `recordable_actions()` as it is replayed. The
//! result is checked by `Game::replay` from scratch: it must end in the victory screen.

use std::fs;

use sts2_core::Character;
use sts2_core::run::RunAction;
use sts2_rl::{CombatMode, RunConfig, RunEnd, RunEnvs, RunOptions, SeedSource};
use sts2_web::Game;

/// A minimal base64 decoder (standard alphabet).
fn base64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0_u32, 0);
    for c in text.bytes().filter(|&c| c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(acc >> bits & 0xFF).expect("a byte"));
        }
    }
    out
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(&format!("\"{key}\":"))? + key.len() + 3;
    Some(&line[at..])
}

#[allow(clippy::print_stdout, reason = "the tool reports what it wrote")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [file, seed, title, out] = &args[..] else {
        panic!("usage: watch_convert EVAL.jsonl SEED TITLE OUT.json");
    };
    let seed: u64 = seed.parse().expect("a numeric seed");
    let text = fs::read_to_string(file).expect("the eval file");
    let line = text
        .lines()
        .find(|l| field(l, "seed").is_some_and(|s| s.starts_with(&format!("{seed},"))))
        .expect("the seed's record");
    let packed = field(line, "actions").expect("a win has actions");
    let packed = packed.strip_prefix('"').expect("a string");
    let packed = &packed[..packed.find('"').expect("a closing quote")];
    let bytes = base64(packed);
    let indices: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect();
    // The env options of the file's meta (`meta.env`).
    let config = RunConfig {
        character: Some(Character::Necrobinder),
        ascension: 10,
        combat: CombatMode::Agent,
        intents: true,
        options: RunOptions::E2E,
    };

    // Replay the agent's indices in the env, recording every engine action.
    let mut envs = RunEnvs::new(
        1,
        SeedSource::List {
            seeds: vec![seed],
            cycle: false,
        },
        config,
    );
    envs.record_actions(true);
    let mut log: Vec<RunAction> = Vec::new();
    let mut ended = None;
    for (t, &a) in indices.iter().enumerate() {
        envs.step_with(&[a], |e| {
            log = e.log.to_vec();
            ended = Some((e.end, e.floor, t));
        })
        .expect("a legal action");
    }
    let (end, floor, t) = ended.expect("the run ends");
    assert_eq!(end, RunEnd::Victory, "the eval run is a win");
    assert_eq!(t + 1, indices.len(), "it ends on its last action");

    // The same actions as web indices.
    let name = format!("RUN{seed}");
    let character = Character::ALL
        .iter()
        .position(|&c| c == Character::Necrobinder)
        .expect("a character index");
    let mut game = Game::new(character, &name, 10, &[]).expect("the web run");
    let mut web: Vec<u32> = Vec::new();
    for action in &log {
        let i = game
            .recordable_actions()
            .iter()
            .position(|a| a == action)
            .unwrap_or_else(|| panic!("web action {} is not offered: {action:?}", web.len() + 1));
        let i = u32::try_from(i).expect("small index");
        game.advance(&[i]).expect("the web takes it");
        web.push(i);
    }

    // Check: from scratch, `Game::replay` ends in the victory screen on the same floor.
    let check = Game::replay(character, &name, 10, &[], &web).expect("replays");
    assert!(check.is_over(), "the replay ends");
    assert_eq!(
        format!("{:?}", check.run().pending_decision()),
        "Victory",
        "the replay is a win"
    );
    let web_floor = check.run().state().total_floor();
    assert_eq!(usize::from(floor), web_floor, "same floor");

    let json = serde_json::json!({
        "title": title,
        "seed": name,
        "character": character,
        "ascension": 10,
        "floor": web_floor,
        "actions": web,
    });
    fs::write(out, format!("{json}\n")).expect("the output file");
    println!(
        "{} agent actions -> {} engine actions, floor {web_floor}, victory",
        indices.len(),
        web.len()
    );
}
