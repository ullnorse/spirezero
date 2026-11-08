//! Project automation, run as `cargo xtask <cmd>`.

mod art;
mod bench;
mod coverage;
mod docs;
mod hits;
mod inventory;
mod json;
mod package;
mod pgo;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Lines of a failing step's output that are shown.
const FAILURE_LINES: usize = 40;

/// Gate steps after [`fmt`]: label and `cargo` arguments.
const GATE: &[(&str, &[&str])] = &[
    (
        "clippy",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
    ("test", &["test", "--workspace"]),
    (
        "wasm",
        &[
            "check",
            "-p",
            "sts2-core",
            "-p",
            "sts2-view",
            "-p",
            "sts2-web",
            "-p",
            "sts2-web-bot",
            "--target",
            "wasm32-unknown-unknown",
        ],
    ),
];

/// Is this one of cargo's `Compiling`/`Checking`/... progress lines?
fn is_progress(line: &str) -> bool {
    let word = line.split_whitespace().next().unwrap_or_default();
    line.starts_with(' ')
        && matches!(
            word,
            "Compiling" | "Checking" | "Downloaded" | "Downloading" | "Blocking" | "Locking"
        )
}

#[allow(
    clippy::print_stdout,
    reason = "xtask is a CLI whose output is its interface"
)]
fn gate() -> ExitCode {
    let (docs_lines, docs_broken) = docs::run();
    if docs_broken {
        for line in docs_lines {
            println!("{line}");
        }
        println!("docs: FAIL");
        return ExitCode::FAILURE;
    }
    println!(
        "docs: ok ({})",
        docs_lines.first().map_or("", String::as_str)
    );
    let mut fmt_check = Command::new("cargo");
    fmt_check.args(["xtask", "fmt", "--check"]);
    let mut steps = vec![("fmt", fmt_check)];
    for (label, args) in GATE {
        let mut command = Command::new("cargo");
        command.args(*args);
        steps.push((label, command));
    }
    for (label, mut command) in steps {
        let Ok(out) = command.output() else {
            println!("{label}: FAIL (could not run cargo)");
            return ExitCode::FAILURE;
        };
        if out.status.success() {
            println!("{label}: ok");
            continue;
        }
        println!("{label}: FAIL");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // Drop cargo's progress lines so the 40 lines are the actual errors.
        let lines: Vec<&str> = text.lines().filter(|line| !is_progress(line)).collect();
        let total = lines.len();
        for line in lines.iter().take(FAILURE_LINES) {
            println!("{line}");
        }
        if total > FAILURE_LINES {
            println!("... {} more lines", total - FAILURE_LINES);
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `cargo xtask fmt [--check]`: `cargo fmt`, then rustfmt on the content entity files
/// (`crates/sts2-core/src/content/<kind>/*.rs`), which `cargo fmt` never visits: their modules
/// are declared only in the registry `build.rs` generates.
#[allow(
    clippy::print_stderr,
    reason = "xtask is a CLI whose output is its interface"
)]
fn fmt(args: &[String]) -> ExitCode {
    let check = args.iter().any(|a| a == "--check");
    let mut cargo = Command::new("cargo");
    cargo.arg("fmt");
    let mut rustfmt = Command::new("rustfmt");
    rustfmt.args(["--edition", "2024"]);
    if check {
        cargo.arg("--check");
        rustfmt.arg("--check");
    }
    let content = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/sts2-core/src/content");
    let files = match entity_files(&content) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("xtask fmt: {}: {e}", content.display());
            return ExitCode::FAILURE;
        }
    };
    rustfmt.args(&files);
    let ok = |command: &mut Command| command.status().is_ok_and(|s| s.success());
    if ok(&mut cargo) && ok(&mut rustfmt) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The `.rs` files one directory below `content`, sorted.
fn entity_files(content: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for kind in std::fs::read_dir(content)? {
        let kind = kind?.path();
        if !kind.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&kind)? {
            let file = file?.path();
            if file.extension().is_some_and(|e| e == "rs") {
                files.push(file);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// `cargo xtask sweep ARGS`: `sts2-diff sweep ARGS` built in the `regress` profile (optimized,
/// debug assertions on, so the engine's self-checks turn into panics the sweep reports).
fn sweep(args: &[String]) -> ExitCode {
    let status = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--profile",
            "regress",
            "-p",
            "sts2-diff",
            "--",
            "sweep",
        ])
        .args(args)
        .status();
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// `cargo xtask lockstep ARGS`: the lockstep oracle (`sts2-lockstep ARGS`: the working tree's
/// engine against the pinned base, step by step) built in the `regress` profile, so the engines'
/// own debug checks run too. The crate is private (it depends on the private repo), so the public
/// export leaves it out.
#[allow(
    clippy::print_stderr,
    reason = "xtask is a CLI; the message is its output"
)]
fn lockstep(args: &[String]) -> ExitCode {
    let krate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/sts2-lockstep");
    if !krate.is_dir() {
        eprintln!(
            "xtask lockstep: crates/sts2-lockstep is not in this checkout (private repo only)"
        );
        return ExitCode::FAILURE;
    }
    let status = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--profile",
            "regress",
            "-p",
            "sts2-lockstep",
            "--",
        ])
        .args(args)
        .status();
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// `cargo xtask art-import ARGS`: [`art::run`] built with optimizations (decoding and
/// drawing hundreds of textures takes minutes in a debug build).
fn art_import(args: &[String]) -> ExitCode {
    let status = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--release",
            "-p",
            "xtask",
            "--",
            "art-import-run",
        ])
        .args(args)
        .status();
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// `cargo xtask bench [--native] [--pgo] [--bolt] ARGS` ([`bench::run`]).
#[allow(
    clippy::print_stderr,
    reason = "xtask is a CLI whose output is its interface"
)]
fn bench(args: &[String]) -> ExitCode {
    match bench::run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask bench: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Prints a command's lines, or its error after `prefix`.
#[allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a CLI whose output is its interface"
)]
fn print_lines(result: Result<Vec<String>, String>, prefix: &str) -> ExitCode {
    match result {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{prefix}{message}");
            ExitCode::FAILURE
        }
    }
}

/// Runs `git` in the workspace and returns stdout, trimmed.
fn git(args: &[&str]) -> Result<String, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let out = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(args)
        .output()
        .map_err(|e| format!("git {}: {e}", args.first().copied().unwrap_or_default()))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// `cargo xtask status`: the mechanical part of `docs/STATUS.md` — branch, worktrees, unmerged
/// task branches, the current referee build and the brief counts.
#[allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a CLI whose output is its interface"
)]
fn status() -> ExitCode {
    match status_lines() {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("xtask status: {message}");
            ExitCode::FAILURE
        }
    }
}

fn status_lines() -> Result<Vec<String>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        Path::to_path_buf,
    );
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    let head = git(&["rev-parse", "--short", "HEAD"])?;
    let dirty = git(&["status", "--porcelain"])?;
    let dirty = if dirty.is_empty() {
        "clean".to_owned()
    } else {
        format!("dirty ({} file(s))", dirty.lines().count())
    };
    let mut lines = vec![format!("{branch} {head} {dirty}")];

    let worktrees = git(&["worktree", "list", "--porcelain"])?;
    let mut others = Vec::new();
    for block in worktrees.split("\n\n") {
        let path = block
            .lines()
            .find_map(|l| l.strip_prefix("worktree "))
            .unwrap_or_default();
        if Path::new(path) != root.as_path() {
            let branch = block
                .lines()
                .find_map(|l| l.strip_prefix("branch refs/heads/"))
                .unwrap_or("detached");
            others.push(format!("{path} ({branch})"));
        }
    }
    lines.push(if others.is_empty() {
        "worktrees: none".to_owned()
    } else {
        format!("worktrees: {}", others.join(", "))
    });

    let mut unmerged = Vec::new();
    for name in git(&[
        "for-each-ref",
        "--format=%(refname:short)",
        "refs/heads/task/",
    ])?
    .lines()
    .filter(|l| !l.is_empty())
    {
        let merged = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["merge-base", "--is-ancestor", name, "main"])
            .status()
            .is_ok_and(|s| s.success());
        if !merged {
            let count = git(&["rev-list", "--count", &format!("main..{name}")])?;
            unmerged.push(format!("{name} ({count})"));
        }
    }
    lines.push(if unmerged.is_empty() {
        "unmerged task branches: none".to_owned()
    } else {
        format!("unmerged task branches: {}", unmerged.join(", "))
    });

    let referee = std::fs::canonicalize(root.join("referee/builds/current"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    lines.push(format!(
        "referee: {}",
        referee
            .as_deref()
            .unwrap_or("no build (run referee/setup.sh)")
    ));

    let active = std::fs::read_dir(root.join("tasks")).map_or(0, |dir| {
        dir.filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .filter(|e| {
                !matches!(
                    e.file_name().to_str(),
                    Some("TEMPLATE.md" | "BACKLOG.md" | "README.md")
                )
            })
            .count()
    });
    let done = std::fs::read_dir(root.join("tasks/done")).map_or(0, |dir| {
        dir.filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .count()
    });
    let backlog = std::fs::read_to_string(root.join("tasks/BACKLOG.md")).map_or(0, |text| {
        text.lines().filter(|l| l.starts_with("- ")).count()
    });
    lines.push(format!(
        "briefs: {active} active, {done} done; backlog: {backlog} items"
    ));
    Ok(lines)
}

/// `cargo xtask merge-check <branch> <hash>`: the branch contains the head a worker reported.
/// UX1 committed on a detached HEAD, so its branch was empty and the merge was a silent no-op.
#[allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a CLI whose output is its interface"
)]
fn merge_check(args: &[String]) -> ExitCode {
    let [branch, hash] = args else {
        eprintln!("usage: cargo xtask merge-check <branch> <head-hash>");
        return ExitCode::FAILURE;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        Path::to_path_buf,
    );
    let exists = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["cat-file", "-e", &format!("{hash}^{{commit}}")])
        .status()
        .is_ok_and(|s| s.success());
    if !exists {
        eprintln!("merge-check: {hash} is not a commit in this repository");
        return ExitCode::FAILURE;
    }
    let contains = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["merge-base", "--is-ancestor", hash, branch])
        .status()
        .is_ok_and(|s| s.success());
    if contains {
        println!("ok: {branch} contains {hash}");
        ExitCode::SUCCESS
    } else {
        eprintln!("merge-check: {branch} does not contain {hash}");
        ExitCode::FAILURE
    }
}

#[allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "xtask is a CLI whose output is its interface"
)]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gate") => gate(),
        Some("fmt") => fmt(&args[1..]),
        Some("status") => status(),
        Some("merge-check") => merge_check(&args[1..]),
        Some("docs-check") => {
            let (lines, broken) = docs::run();
            for line in lines {
                println!("{line}");
            }
            if broken {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("inventory") => print_lines(inventory::run(&args[1..]), ""),
        Some("hits") => match hits::run(&args[1..]) {
            Ok((lines, failed)) => {
                for line in lines {
                    println!("{line}");
                }
                if failed {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        },
        Some("coverage") => match coverage::run(&args[1..]) {
            Ok((lines, missing)) => {
                for line in lines {
                    println!("{line}");
                }
                if missing {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        },
        Some("art-import") => art_import(&args[1..]),
        Some("art-import-run") => print_lines(art::run(&args[1..]), "xtask art-import: "),
        Some("package") => match package::run(&args[1..]) {
            Ok((lines, ok)) => {
                for line in lines {
                    println!("{line}");
                }
                if ok {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(message) => {
                eprintln!("xtask package: {message}");
                ExitCode::FAILURE
            }
        },
        Some("pgo") => print_lines(pgo::run(&args[1..]), "xtask pgo: "),
        Some("sweep") => sweep(&args[1..]),
        Some("lockstep") => lockstep(&args[1..]),
        Some("bench") => bench(&args[1..]),
        _ => {
            println!(
                "usage: cargo xtask <gate|fmt|status|merge-check|docs-check|inventory|coverage|hits|sweep|lockstep|bench|pgo|art-import|package>"
            );
            ExitCode::FAILURE
        }
    }
}
