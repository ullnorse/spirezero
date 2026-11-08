//! `cargo xtask bench [--native] [--pgo] [--bolt] ARGS`: `sts2-bench ARGS` (random-play fights and
//! whole runs per second; the flags are documented in its source) built in the `fast` profile
//! (fat LTO, one codegen unit, `panic = "abort"`).
//!
//! - `--native` compiles for this machine's CPU (`-Ctarget-cpu=native`), in `target/native/`.
//! - `--pgo` makes the build profile-guided ([`crate::pgo`]: an instrumented build plays the
//!   training bench, other seeds than the default ones), and the bench runs on the build that
//!   uses the profile (`target/[native/]pgo/{gen,use}/`). The plain LTO build stays the
//!   reference for instruction counts in perf work: PGO's layout moves with every change.
//! - `--bolt` optimizes the final binary's layout with BOLT: it is linked with `--emit-relocs`,
//!   instrumented by `llvm-bolt`, plays the training bench, and is rewritten from that profile
//!   (`<build dir>/bolt/`). `llvm-bolt` is `$LLVM_BOLT`, or the newest `llvm-bolt[-N]` on `PATH`
//!   (Ubuntu: `apt install bolt-22`).
//!
//! Every variant has its own target directory, so switching between them does not rebuild.

use std::process::{Command, Stdio};

use crate::pgo::{self, Build, TRAINING, exec};

#[derive(Clone, Copy, Debug)]
struct Variant {
    native: bool,
    pgo: bool,
    bolt: bool,
}

/// Builds the variant `args` asks for and runs the bench with the other arguments.
pub fn run(args: &[String]) -> Result<(), String> {
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let variant = Variant {
        native: has("--native"),
        pgo: has("--pgo"),
        bolt: has("--bolt"),
    };
    let args: Vec<&String> = args
        .iter()
        .filter(|a| !matches!(a.as_str(), "--native" | "--pgo" | "--bolt"))
        .collect();
    let binary = build(variant)?;
    exec(Command::new(binary).args(args))
}

/// Builds `variant`; the path of its binary.
fn build(variant: Variant) -> Result<String, String> {
    let target = format!("{}/../target", env!("CARGO_MANIFEST_DIR"));
    let root = if variant.native {
        format!("{target}/native")
    } else {
        target
    };
    let mut flags: Vec<String> = Vec::new();
    if variant.native {
        flags.push("-Ctarget-cpu=native".into());
    }
    if variant.pgo {
        let training = Build {
            profile: "fast",
            target: None,
            flags: &flags,
            default_features: true,
        };
        let profile = pgo::profile(&format!("{root}/pgo"), &training)?;
        flags.push(pgo::use_flag(&profile));
    }
    if variant.bolt {
        flags.push("-Clink-arg=-Wl,--emit-relocs".into());
    }
    let dir = match (variant.pgo, variant.bolt) {
        (false, false) => root,
        (false, true) => format!("{root}/bolt"),
        (true, false) => format!("{root}/pgo/use"),
        (true, true) => format!("{root}/pgo/use-bolt"),
    };
    let build = Build {
        profile: "fast",
        target: None,
        flags: &flags,
        default_features: true,
    };
    let binary = build.bench(&dir, &[])?;
    if variant.bolt {
        return bolt(&binary, &format!("{dir}/bolt"));
    }
    Ok(binary)
}

/// Plays [`TRAINING`] on `binary`.
fn train(binary: &str) -> Result<(), String> {
    exec(Command::new(binary).args(TRAINING).stdout(Stdio::null()))
}

/// Instruments `binary` with BOLT in `dir`, trains it and writes the optimized binary; its path.
fn bolt(binary: &str, dir: &str) -> Result<String, String> {
    let llvm_bolt = llvm_bolt()?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    let instrumented = format!("{dir}/instrumented");
    let profile = format!("{dir}/profile.fdata");
    let optimized = format!("{dir}/sts2-bench");
    let _ = std::fs::remove_file(&profile);
    let log = |name: &str| -> Result<Stdio, String> {
        let path = format!("{dir}/{name}.log");
        std::fs::File::create(&path)
            .map(Stdio::from)
            .map_err(|e| format!("{path}: {e}"))
    };
    exec(
        Command::new(&llvm_bolt)
            .args([binary, "-instrument", "-o", &instrumented])
            .arg(format!("--instrumentation-file={profile}"))
            .stdout(log("instrument")?)
            .stderr(log("instrument-err")?),
    )?;
    train(&instrumented)?;
    exec(
        Command::new(&llvm_bolt)
            .args([binary, "-o", &optimized])
            .arg(format!("-data={profile}"))
            .args([
                "-reorder-blocks=ext-tsp",
                "-reorder-functions=cdsort",
                "-split-functions",
                "-split-all-cold",
                "-split-eh",
                "-icf=1",
                "-use-gnu-stack",
            ])
            .stdout(log("optimize")?)
            .stderr(log("optimize-err")?),
    )?;
    Ok(optimized)
}

/// `$LLVM_BOLT`, or the newest `llvm-bolt[-N]` that runs.
fn llvm_bolt() -> Result<String, String> {
    let runs = |name: &str| {
        Command::new(name)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if let Ok(path) = std::env::var("LLVM_BOLT") {
        return Ok(path);
    }
    std::iter::once("llvm-bolt".to_owned())
        .chain((14..=30).rev().map(|n| format!("llvm-bolt-{n}")))
        .find(|name| runs(name))
        .ok_or_else(|| {
            "no llvm-bolt on PATH (Ubuntu: `apt install bolt-22`; or set LLVM_BOLT)".into()
        })
}
