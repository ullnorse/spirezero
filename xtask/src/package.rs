//! `cargo xtask package [TARGET...]`: the TUI as one self-contained binary per OS, with the
//! game text and art from `cargo xtask art-import` built in (`sts2-tui`'s `embedded` feature),
//! in `target/package/`. Personal use only: the binaries hold the game's assets, so they are
//! never committed or uploaded. How to run them, and the macOS steps: `docs/packaging.md`.
//!
//! `cargo xtask package [--no-pgo] [TARGET...]`. A target whose binary runs here is
//! profile-guided ([`crate::pgo`]): an instrumented `sts2-bench` built for that target, in the
//! `portable` profile and with the system allocator like the TUI, plays the training bench, and
//! the TUI is built with its profile (the engine's code; the TUI's own is not trained). On x86-64 Linux that is the Linux binary; the
//! others are plain fat-LTO builds, since a profile applies only to builds of its own target
//! (`--no-pgo`: all plain).
//!
//! Targets (default: all) are built with the `portable` profile:
//! - `linux`: `x86_64-unknown-linux-musl`, static (`rustup target add`; plain cargo links it).
//! - `windows`: `x86_64-pc-windows-gnu` (`rustup target add`, and `mingw-w64` for the linker).
//! - `macos-arm`, `macos-x86`: `aarch64-apple-darwin`, `x86_64-apple-darwin` through
//!   `cargo zigbuild` (`cargo install cargo-zigbuild`, and `zig`); no macOS SDK is needed, and
//!   zig's linker signs the binaries ad hoc, which Apple silicon requires.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pgo::{self, Build};

/// One packaged binary.
#[derive(Debug)]
struct Target {
    name: &'static str,
    triple: &'static str,
    /// Built with `cargo zigbuild` instead of `cargo build`.
    zig: bool,
    /// The packaged file's name.
    file: &'static str,
}

const TARGETS: [Target; 4] = [
    Target {
        name: "linux",
        triple: "x86_64-unknown-linux-musl",
        zig: false,
        file: "sts2-tui-linux-x86_64",
    },
    Target {
        name: "windows",
        triple: "x86_64-pc-windows-gnu",
        zig: false,
        file: "sts2-tui-windows-x86_64.exe",
    },
    Target {
        name: "macos-arm",
        triple: "aarch64-apple-darwin",
        zig: true,
        file: "sts2-tui-macos-arm64",
    },
    Target {
        name: "macos-x86",
        triple: "x86_64-apple-darwin",
        zig: true,
        file: "sts2-tui-macos-x86_64",
    },
];

const PROFILE: &str = "portable";

/// Lines of a failed build's output shown.
const FAILURE_LINES: usize = 15;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is in the workspace")
        .to_owned()
}

/// What the build needs from the import, checked first so a missing import is one line.
fn check_import() -> Result<(), String> {
    let data = crate::art::data_dir()?;
    for needed in [data.join("art").join("art.bundle"), data.join("text")] {
        if !needed.exists() {
            return Err(format!(
                "{} is missing: run `cargo xtask art-import` first",
                needed.display()
            ));
        }
    }
    Ok(())
}

impl Target {
    /// Does this target's binary run on `host` (so it can play the PGO training bench)?
    fn runs_on(&self, host: &str) -> bool {
        self.triple == host
            || (host == "x86_64-unknown-linux-gnu" && self.triple == "x86_64-unknown-linux-musl")
    }
}

/// The PGO profile for `target`: its instrumented bench trained in `target/pgo-package/`.
fn train(target: &Target) -> Result<String, String> {
    let dir = root()
        .join("target")
        .join("pgo-package")
        .join(target.triple);
    let build = Build {
        profile: PROFILE,
        target: Some(target.triple),
        flags: &[],
        default_features: false,
    };
    pgo::profile(&dir.to_string_lossy(), &build)
}

/// Builds `target`, with the PGO profile `profile` if given; on success the packaged file's
/// size in bytes.
fn build(target: &Target, profile: Option<&str>, out: &Path) -> Result<u64, String> {
    let mut command = Command::new("cargo");
    if let Some(profile) = profile {
        command.env("RUSTFLAGS", pgo::use_flag(profile));
    }
    command
        .arg(if target.zig { "zigbuild" } else { "build" })
        .args([
            "--profile",
            PROFILE,
            "-p",
            "sts2-tui",
            "--features",
            "embedded",
        ])
        .args(["--target", target.triple])
        .current_dir(root());
    let output = command
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = stderr.lines().collect();
        let tail = lines[lines.len().saturating_sub(FAILURE_LINES)..].join("\n");
        let hint = if target.zig {
            "needs `rustup target add`, zig and `cargo install cargo-zigbuild`"
        } else {
            "needs `rustup target add` (and mingw-w64 for windows)"
        };
        return Err(format!("build failed ({hint}):\n{tail}"));
    }
    let binary = if target.triple.contains("windows") {
        "sts2-tui.exe"
    } else {
        "sts2-tui"
    };
    let built = root()
        .join("target")
        .join(target.triple)
        .join(PROFILE)
        .join(binary);
    let packaged = out.join(target.file);
    std::fs::copy(&built, &packaged).map_err(|e| format!("{}: {e}", built.display()))
}

/// Builds the targets named in `args` (default: all); one line per target, and whether all
/// were built.
pub fn run(args: &[String]) -> Result<(Vec<String>, bool), String> {
    let use_pgo = !args.iter().any(|a| a == "--no-pgo");
    let args: Vec<&String> = args.iter().filter(|a| *a != "--no-pgo").collect();
    let wanted: Vec<&Target> = if args.is_empty() {
        TARGETS.iter().collect()
    } else {
        args.iter()
            .map(|name| {
                TARGETS
                    .iter()
                    .find(|t| t.name == *name || t.triple == *name)
                    .ok_or_else(|| {
                        let names: Vec<&str> = TARGETS.iter().map(|t| t.name).collect();
                        format!("unknown target {name} (have {})", names.join(", "))
                    })
            })
            .collect::<Result<_, _>>()?
    };
    check_import()?;
    let host = pgo::host()?;
    let out = root().join("target").join("package");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut lines = Vec::new();
    let mut ok = true;
    for target in wanted {
        let pgo = use_pgo && target.runs_on(&host);
        let built = if pgo {
            train(target).and_then(|profile| build(target, Some(&profile), &out))
        } else {
            build(target, None, &out)
        };
        match built {
            Ok(size) => lines.push(format!(
                "{:<28} {:>5.1} MB  ({}, {})",
                target.file,
                size_mb(size),
                target.triple,
                if pgo { "PGO" } else { "LTO" }
            )),
            Err(message) => {
                ok = false;
                lines.push(format!("{:<28} FAIL  {message}", target.file));
            }
        }
    }
    lines.push(format!("in {}", out.display()));
    Ok((lines, ok))
}

#[allow(
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    reason = "a file size for display, not game logic"
)]
fn size_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
