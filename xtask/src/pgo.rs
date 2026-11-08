//! Profile-guided builds (PGO), shared by `cargo xtask bench --pgo`, `cargo xtask package` and
//! `cargo xtask pgo`.
//!
//! A profile is made by building `sts2-bench` instrumented (`-Cprofile-generate`), playing the
//! training bench ([`TRAINING`]) on it, and merging what it counted with `llvm-profdata` (rustup
//! component `llvm-tools`). Only the training run's own counts are merged: the build scripts and
//! proc macros the instrumented build compiles write profiles too, and those would vary with
//! what cargo happened to rebuild.
//!
//! LLVM matches a profile to a function by its symbol name, and cargo's `-C metadata` (part of
//! every symbol) depends on the cargo profile and on whether `--target` was given. So a profile
//! only applies to builds with the same cargo profile and the same `--target` (or none) as its
//! training build, in this workspace; any other build ignores it without a warning (`fast`'s
//! profile made a `portable` build 1.7% *slower* in instructions, PF15). That is why a target
//! must be trained by running its own instrumented build: `package` can train the Linux binary
//! here, not the Windows or macOS ones.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The training bench: the default workload's mix (fights and Necrobinder runs) on other seeds,
/// one thread, so its counts are the same every time. A broader set (1,000 runs of every
/// character) measured 4% slower on fights and 3% faster only on runs of other characters
/// (`docs/perf.md`, PF7).
pub const TRAINING: &[&str] = &[
    "--seeds",
    "220",
    "--reps",
    "10",
    "--runs",
    "100",
    "--seed-start",
    "1000000",
];

/// How a profile's builds are made: the cargo profile, the target (`None`: no `--target`), extra
/// `RUSTFLAGS`, and whether the bench's default features are on (off, it uses the system
/// allocator). Always `cargo build`, never `cargo zigbuild`: zig's linker can't link an
/// instrumented binary, and a training build runs on this machine anyway.
#[derive(Clone, Copy, Debug)]
pub struct Build<'a> {
    pub profile: &'a str,
    pub target: Option<&'a str>,
    pub flags: &'a [String],
    pub default_features: bool,
}

impl Build<'_> {
    /// `cargo build -p sts2-bench` into `dir` with `flags` added to [`Self::flags`] as
    /// `RUSTFLAGS`; the binary's path.
    pub fn bench(&self, dir: &str, flags: &[String]) -> Result<String, String> {
        let mut command = Command::new("cargo");
        command
            .args(["build", "--quiet"])
            .args(["--profile", self.profile, "-p", "sts2-bench"])
            .env("RUSTFLAGS", [self.flags, flags].concat().join(" "))
            .env("CARGO_TARGET_DIR", dir);
        if !self.default_features {
            command.arg("--no-default-features");
        }
        let mut out = PathBuf::from(dir);
        if let Some(target) = self.target {
            command.args(["--target", target]);
            out.push(target);
        }
        exec(&mut command)?;
        Ok(out
            .join(self.profile)
            .join("sts2-bench")
            .display()
            .to_string())
    }
}

/// Builds the instrumented bench under `dir/gen`, trains it, and merges the profile into
/// `dir/merged.profdata`; its path.
pub fn profile(dir: &str, build: &Build<'_>) -> Result<String, String> {
    let raw = format!("{dir}/raw");
    let profile = format!("{dir}/merged.profdata");
    let _ = std::fs::remove_dir_all(&raw);
    std::fs::create_dir_all(&raw).map_err(|e| format!("{raw}: {e}"))?;
    let profdata = llvm_profdata()?;
    let flags = [
        format!("-Cprofile-generate={raw}"),
        "-Cllvm-args=-vp-counters-per-site=8".into(),
    ];
    let binary = build.bench(&format!("{dir}/gen"), &flags)?;
    let trained = format!("{raw}/training.profraw");
    exec(
        Command::new(binary)
            .args(TRAINING)
            .env("LLVM_PROFILE_FILE", &trained)
            .stdout(Stdio::null()),
    )?;
    exec(Command::new(&profdata).args(["merge", "-o", &profile, &trained]))?;
    Ok(profile)
}

/// The `RUSTFLAGS` entry that makes a build use `profile`.
pub fn use_flag(profile: &str) -> String {
    format!("-Cprofile-use={profile}")
}

/// The host's target triple.
pub fn host() -> Result<String, String> {
    let version = rustc("-vV")?;
    version
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "no host in `rustc -vV`".into())
}

fn rustc(arg: &str) -> Result<String, String> {
    let out = Command::new("rustc")
        .arg(arg)
        .output()
        .map_err(|e| format!("rustc {arg}: {e}"))?;
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

/// The rustup `llvm-profdata` of the active toolchain.
fn llvm_profdata() -> Result<String, String> {
    let sysroot = rustc("--print=sysroot")?;
    let profdata = format!(
        "{}/lib/rustlib/{}/bin/llvm-profdata",
        sysroot.trim(),
        host()?
    );
    if Path::new(&profdata).exists() {
        Ok(profdata)
    } else {
        Err(format!(
            "{profdata} is missing: `rustup component add llvm-tools`"
        ))
    }
}

/// Runs `command`; an error unless it exits successfully.
pub fn exec(command: &mut Command) -> Result<(), String> {
    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{command:?} failed: {status}")),
        Err(e) => Err(format!("{command:?}: {e}")),
    }
}

/// `cargo xtask pgo [--profile P] [--target T]`: a profile for a binary of this workspace that
/// uses `sts2-core` and wants its speed (a search, a trainer), built with cargo profile `P`
/// (default `fast`) and `--target T` (default: none), in `target/pgo-lib/`; the lines saying how
/// to build with it.
pub fn run(args: &[String]) -> Result<Vec<String>, String> {
    let mut profile = "fast";
    let mut target = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--profile" => profile = value,
            "--target" => target = Some(value.as_str()),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let name = target.map_or_else(|| profile.to_owned(), |t| format!("{profile}-{t}"));
    let dir = format!("{}/../target/pgo-lib/{name}", env!("CARGO_MANIFEST_DIR"));
    let build = Build {
        profile,
        target,
        flags: &[],
        default_features: true,
    };
    let path = profile_path(&dir, &build)?;
    let target_arg = target.map_or_else(String::new, |t| format!(" --target {t}"));
    Ok(vec![
        format!("profile: {path}"),
        format!(
            "build with: RUSTFLAGS=\"{}\" cargo build --profile {profile}{target_arg} -p <your crate>",
            use_flag(&path)
        ),
        "(the same cargo profile and --target as here, or the profile is silently ignored)".into(),
    ])
}

/// [`profile`] with its path made absolute, so it can be pasted into another build.
fn profile_path(dir: &str, build: &Build<'_>) -> Result<String, String> {
    let path = profile(dir, build)?;
    std::fs::canonicalize(&path)
        .map(|p| p.display().to_string())
        .map_err(|e| format!("{path}: {e}"))
}
