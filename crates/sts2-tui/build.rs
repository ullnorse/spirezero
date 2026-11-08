//! With the `embedded` feature: writes `$OUT_DIR/embedded.rs`, which builds the import's game
//! text (deflated, one file per table) into the binary.
//!
//! The text comes from `$SPIREZERO_DATA` (or the old `$SPIREBOT_DATA`), default
//! `~/.cache/spirezero-data` (`~/.cache/spirebot-data` while only that exists), where `cargo xtask
//! art-import` writes `text/*.json`. Without it the build fails and says so.
#![allow(
    clippy::print_stdout,
    reason = "cargo reads a build script's instructions from its stdout"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    for var in ["SPIREZERO_DATA", "SPIREBOT_DATA"] {
        println!("cargo::rerun-if-env-changed={var}");
        if let Some(dir) = std::env::var_os(var) {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .expect("the `embedded` feature needs HOME (or SPIREZERO_DATA) to find the import");
    let cache = PathBuf::from(home).join(".cache");
    let (new, old) = (cache.join("spirezero-data"), cache.join("spirebot-data"));
    if !new.exists() && old.is_dir() {
        old
    } else {
        new
    }
}

fn missing(what: &Path) -> ! {
    panic!(
        "the `embedded` feature needs the game's text from `cargo xtask art-import`, \
         and {} is missing: run the import first (or set SPIREZERO_DATA to its directory)",
        what.display()
    )
}

/// `path` as a string literal for `include_bytes!`.
fn literal(path: &Path) -> String {
    format!("{:?}", path.to_str().expect("UTF-8 path"))
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_EMBEDDED").is_none() {
        return;
    }
    let data = data_dir();
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));

    let text_dir = data.join("text");
    println!("cargo::rerun-if-changed={}", text_dir.display());
    let mut tables: Vec<PathBuf> = std::fs::read_dir(&text_dir)
        .unwrap_or_else(|_| missing(&text_dir))
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    tables.sort();
    if tables.is_empty() {
        missing(&text_dir.join("*.json"));
    }
    let mut code = String::from("/// `(table, deflated JSON)` of every imported text table.\n");
    code.push_str("pub static TEXT: &[(&str, &[u8])] = &[\n");
    for path in &tables {
        println!("cargo::rerun-if-changed={}", path.display());
        let json = std::fs::read(path).unwrap_or_else(|_| missing(path));
        let table = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("UTF-8 name");
        let deflated = out.join(format!("{table}.json.deflate"));
        std::fs::write(&deflated, miniz_oxide::deflate::compress_to_vec(&json, 10))
            .expect("write to OUT_DIR");
        let _ = writeln!(
            code,
            "    ({table:?}, include_bytes!({})),",
            literal(&deflated)
        );
    }
    code.push_str("];\n");
    std::fs::write(out.join("embedded.rs"), code).expect("write to OUT_DIR");
}
