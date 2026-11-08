//! Compiles the source hash in (`STS2_SOURCE_HASH`): the first 16 hex digits of SHA-256 over
//! every file the library is built from, so a trainer can refuse a library built from other sources than its checkout's
//! (`python/sts2/ffi.py` computes the same hash and compares it with `sts2_build_id()`).
//!
//! The files are the workspace's `Cargo.toml` and `Cargo.lock` and every file under each of
//! [`CRATES`]' `Cargo.toml`, `build.rs`, `src/` and `vocab/`, sorted by their path from the
//! workspace root (`/`-separated, bytewise). Each file adds its path, a 0 byte, its length (`u64`
//! little-endian) and its contents to the hashed stream.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

use sha2::{Digest, Sha256};

/// The crates the library is built from (in the workspace).
const CRATES: [&str; 4] = ["sts2-core", "sts2-macros", "sts2-rl", "sts2-ffi"];

/// The entries of each crate that are hashed.
const ENTRIES: [&str; 4] = ["Cargo.toml", "build.rs", "src", "vocab"];

/// Every file under `path` (or `path` itself), into `out`.
fn walk(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let entries = fs::read_dir(path).expect("a readable source directory");
        for entry in entries {
            walk(&entry.expect("a directory entry").path(), out);
        }
    } else if path.is_file() {
        out.push(path.to_owned());
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .expect("the crate is in crates/ of the workspace");
    let mut roots = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    for name in CRATES {
        let dir = root.join("crates").join(name);
        // The whole crate directory, so a file that appears is seen too.
        println!("cargo::rerun-if-changed={}", dir.display());
        roots.extend(ENTRIES.iter().map(|entry| dir.join(entry)));
    }
    println!("cargo::rerun-if-changed={}", roots[0].display());
    println!("cargo::rerun-if-changed={}", roots[1].display());
    let mut files = Vec::new();
    for path in &roots {
        walk(path, &mut files);
    }
    let mut named: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(root).expect("under the root");
            let name = relative
                .components()
                .map(|c| c.as_os_str().to_str().expect("a UTF-8 path"))
                .collect::<Vec<_>>()
                .join("/");
            (name, path)
        })
        .collect();
    named.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut sha = Sha256::new();
    for (name, path) in &named {
        let contents = fs::read(path).expect("a readable source file");
        let len = u64::try_from(contents.len()).expect("a file below 2^64 bytes");
        sha.update(name.as_bytes());
        sha.update([0]);
        sha.update(len.to_le_bytes());
        sha.update(&contents);
    }
    let hash = sha
        .finalize()
        .iter()
        .take(8)
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").expect("a string");
            hex
        });
    println!("cargo::rustc-env=STS2_SOURCE_HASH={hash}");
    println!("cargo::rustc-env=STS2_SOURCE_FILES={}", named.len());
}
