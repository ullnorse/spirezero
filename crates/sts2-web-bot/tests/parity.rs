//! WEB82: the browser's combat-net path (the Rust encoder and forward pass) against the logits
//! of `PyTorch` on the human panel's fights (`python/web82_export.py dump`).
//!
//! The rows and the weights are local data (`~/.cache/spirebot-data/web82/parity.bin`, the
//! human panel's states, never committed; `~/.cache/spirebot-data/net/cb3.bin`); the test
//! says so and passes when they are missing. Run it with `--release` (800 forward passes).

#![allow(
    clippy::float_arithmetic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "a numeric comparison test that reports what it found"
)]

use std::path::PathBuf;

use sts2_web_bot::net::Net;
use sts2_web_bot::net::encode::{ACTIONS, encode};

fn data(rel: &str) -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME");
    PathBuf::from(home).join(".cache/spirebot-data").join(rel)
}

fn u32_at(b: &[u8], at: usize) -> usize {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "800 forward passes take minutes unoptimized: cargo test --release -p sts2-web-bot"
)]
fn rust_path_matches_pytorch() {
    let (rows_path, net_path) = (data("web82/parity.bin"), data("net/cb3.bin"));
    let (Ok(rows), Ok(weights)) = (std::fs::read(&rows_path), std::fs::read(&net_path)) else {
        println!("parity: no local data ({rows_path:?}, {net_path:?}): skipped");
        return;
    };
    let net = Net::from_bytes(&weights).unwrap();
    assert_eq!(&rows[..4], b"CB3P");
    let (n, width, acts) = (u32_at(&rows, 4), u32_at(&rows, 8), u32_at(&rows, 12));
    assert_eq!(acts, ACTIONS);
    let stride = width + 6 * 8 + acts * 4 + 4;
    let (mut same, mut worst, mut over) = (0, 0.0f32, 0);
    for i in 0..n {
        let r = &rows[16 + i * stride..][..stride];
        let obs = &r[..width];
        let mask: Vec<u64> = (0..6)
            .map(|k| u64::from_le_bytes(r[width + 8 * k..width + 8 * k + 8].try_into().unwrap()))
            .collect();
        let want: Vec<f32> = (0..acts)
            .map(|a| {
                let at = width + 48 + 4 * a;
                f32::from_le_bytes(r[at..at + 4].try_into().unwrap())
            })
            .collect();
        let tok = encode(obs, &mask).unwrap();
        let got = net.logits(&tok);
        let mut row_worst = 0.0f32;
        for a in 0..acts {
            let legal = want[a] > -1e29;
            assert_eq!(
                legal,
                got[a].is_finite(),
                "row {i} index {a}: legality differs"
            );
            if legal {
                row_worst = row_worst.max((want[a] - got[a]).abs());
            }
        }
        worst = worst.max(row_worst);
        over += usize::from(row_worst > 1e-3);
        let argmax = |l: &[f32]| {
            (0..acts)
                .filter(|&a| l[a].is_finite() && l[a] > -1e29)
                .max_by(|&a, &b| l[a].total_cmp(&l[b]).then(b.cmp(&a)))
        };
        same += usize::from(argmax(&want) == argmax(&got));
    }
    let agree = same as f64 / n as f64;
    println!(
        "parity: {n} rows, argmax agrees on {same} ({:.2}%), max |logit diff| {worst:.2e}, rows over 1e-3: {over}",
        100.0 * agree
    );
    assert!(n >= 500, "only {n} rows");
    assert!(agree >= 0.995, "argmax agreement {agree}");
    assert!(worst <= 1e-3, "logits differ by {worst}");
}
