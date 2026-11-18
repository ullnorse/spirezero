//! WEB82: the browser path's combat net (`RunSight`, the Rust encoder and forward pass, greedy)
//! playing the human panel's boss and elite fights natively, for its death rate next to
//! `tools/cb3/panel.sh`'s (`PyTorch`). Local data only.
//!
//! `cargo run --release -p sts2-web-bot --example cb3_fights -- [FIGHTS_PER_ROOM] [THREADS]`

#![allow(
    clippy::print_stdout,
    clippy::float_arithmetic,
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::expect_used,
    reason = "a measuring tool that prints what it found"
)]

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use sts2_core::run::RunDecision;
use sts2_rl::HumanStart;
use sts2_rl::human::Variant;
use sts2_rl::sight::RunSight;
use sts2_web_bot::net::Net;
use sts2_web_bot::net::encode::encode;

fn data(rel: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME"))
        .join(".cache/spirebot-data")
        .join(rel)
}

/// Plays one fight; (died, decisions the net was asked).
fn fight(net: &Net, start: &HumanStart, seed: u64) -> Result<(bool, usize), String> {
    let mut run = start.build(seed, Variant::default(), 0)?;
    let mut sight = RunSight::new(RunSight::cb3());
    let mut asked = 0;
    for _ in 0..5_000 {
        if run.combat().is_none() || !matches!(run.pending_decision(), RunDecision::Combat(_)) {
            break;
        }
        let classes = sight.settle(&run);
        let index = match classes {
            0 => return Err("stuck".into()),
            1 => sight.mask().nth(0).ok_or("no bit")?,
            _ => {
                sight.see(&run);
                asked += 1;
                let tok = encode(sight.obs(), sight.mask().bits()).map_err(|e| format!("{e:?}"))?;
                Net::greedy(&net.logits(&tok)).ok_or("no legal logit")?
            }
        };
        let action = sight.action(index).ok_or("no action")?.clone();
        run.apply(action).map_err(|e| format!("{e:?}"))?;
    }
    Ok((
        matches!(run.pending_decision(), RunDecision::GameOver),
        asked,
    ))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let per_room: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(24);
    let threads: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(4);
    let net = Net::from_bytes(&std::fs::read(data("net/cb3.bin")).expect("weights")).expect("net");
    let text = std::fs::read_to_string(data("cb1/panel_bench.jsonl")).expect("panel bench");
    let mut jobs = Vec::new();
    for room in ["_BOSS", "_ELITE"] {
        let recs: Vec<&str> = text
            .lines()
            .filter(|l| {
                serde_json::from_str::<serde_json::Value>(l)
                    .ok()
                    .and_then(|v| v["encounter"].as_str().map(|e| e.ends_with(room)))
                    .unwrap_or(false)
            })
            .collect();
        let step = (recs.len() / per_room).max(1);
        for (k, line) in recs.iter().step_by(step).take(per_room).enumerate() {
            jobs.push((room, *line, 7_000_000 + k as u64));
        }
    }
    let results = Mutex::new(Vec::new());
    let cursor = Mutex::new(0usize);
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = cursor.lock().expect("lock");
                        *n += 1;
                        *n - 1
                    };
                    let Some(&(room, line, seed)) = jobs.get(i) else {
                        break;
                    };
                    let out = HumanStart::from_json(line).and_then(|st| fight(&net, &st, seed));
                    results.lock().expect("lock").push((room, out));
                }
            });
        }
    });
    let results = results.into_inner().expect("lock");
    let mut asked = 0;
    for room in ["_BOSS", "_ELITE"] {
        let rs: Vec<_> = results.iter().filter(|r| r.0 == room).collect();
        let ok: Vec<_> = rs.iter().filter_map(|r| r.1.as_ref().ok()).collect();
        let died = ok.iter().filter(|r| r.0).count();
        asked += ok.iter().map(|r| r.1).sum::<usize>();
        println!(
            "{}: {} fights, {} died ({:.1}%), {} errors",
            room.trim_start_matches('_').to_lowercase(),
            ok.len(),
            died,
            100.0 * died as f64 / ok.len().max(1) as f64,
            rs.len() - ok.len()
        );
    }
    let secs = t0.elapsed().as_secs_f64();
    println!(
        "{asked} net decisions in {secs:.0} s on {threads} threads ({:.1} ms each per thread)",
        1000.0 * secs * threads as f64 / asked.max(1) as f64
    );
}
