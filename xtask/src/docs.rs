//! `cargo xtask docs-check`: the relative links in the active docs resolve, and the entry points
//! exist. Dated records (`docs/history/`, `tasks/done/`) are archives: their paths describe where
//! things lived then and are not checked.
//!
//! Absolute links, `#anchors` and links with a title (`(path "title")`) are ignored; a target is
//! checked after dropping its `#fragment`.

use std::path::{Path, PathBuf};

/// Directories that are never docs (or hold generated content).
const SKIP_DIRS: &[&str] = &[
    "target",
    ".git",
    "ref",
    "builds",
    "vendor",
    "node_modules",
    ".venv",
];

/// Files that are the active doc entry points.
const REQUIRED: &[&str] = &[
    "README.md",
    "docs/README.md",
    "docs/CHANGELOG.md",
    "docs/regression.md",
    "docs/semantics.md",
];

/// Paths that exist only in the private repo (the public export leaves them out, see
/// `release/manifest.toml`). Each is checked like the others when present; when absent it is not
/// required and links into it are not reported.
const PRIVATE_ONLY: &[&str] = &[
    "CLAUDE.md",
    "docs/STATUS.md",
    "docs/PLAN.md",
    "docs/WORKFLOW.md",
    "docs/decisions.md",
    "docs/devlog.md",
    "docs/reviews",
    "docs/history",
    "tasks",
    "ref",
    "referee/vendor",
    "tools/mentor_crosscheck",
    "crates/sts2-lockstep",
];

/// Entry points required when the private-only area they live in is present.
const REQUIRED_PRIVATE: &[&str] = &[
    "CLAUDE.md",
    "docs/STATUS.md",
    "docs/PLAN.md",
    "docs/WORKFLOW.md",
    "tasks/README.md",
    "tasks/TEMPLATE.md",
    "tasks/BACKLOG.md",
];

/// Whether `path` lies in a private-only area that is absent from this checkout.
fn absent_private(root: &Path, path: &Path) -> bool {
    let Some(rel) = normalize(path).and_then(|p| p.strip_prefix(root).ok().map(Path::to_path_buf))
    else {
        return false;
    };
    PRIVATE_ONLY
        .iter()
        .any(|area| rel.starts_with(area) && !root.join(area).exists())
}

/// `path` with `.` and `..` components resolved lexically.
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !SKIP_DIRS.contains(&name) {
                walk(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Whether a file is an archive whose paths may be stale.
fn archived(root: &Path, file: &Path) -> bool {
    let Ok(rel) = file.strip_prefix(root) else {
        return true;
    };
    let rel = rel.to_string_lossy().replace('\\', "/");
    rel.starts_with("docs/history/") || rel.starts_with("tasks/done/")
}

/// The link targets on one line (`](target)`), without the title or fragment.
fn targets(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let target = rest[..end].trim();
        rest = &rest[end + 1..];
        let target = target
            .strip_prefix('<')
            .and_then(|t| t.strip_suffix('>'))
            .unwrap_or(target);
        let target = target.split_whitespace().next().unwrap_or_default();
        let target = target.split('#').next().unwrap_or_default();
        if !target.is_empty()
            && !target.starts_with("http://")
            && !target.starts_with("https://")
            && !target.starts_with("mailto:")
            && !target.starts_with('/')
            && !target.starts_with('#')
        {
            out.push(target);
        }
    }
    out
}

/// Run the check; returns the lines to print and whether anything is broken.
pub fn run() -> (Vec<String>, bool) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        Path::to_path_buf,
    );
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let mut broken = Vec::new();
    let private = REQUIRED_PRIVATE
        .iter()
        .filter(|path| !absent_private(&root, &root.join(path)));
    for required in REQUIRED.iter().chain(private) {
        if !root.join(required).is_file() {
            broken.push(format!("missing {required}"));
        }
    }
    let mut checked = 0;
    for file in &files {
        if archived(&root, file) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            for target in targets(line) {
                let path = file.parent().unwrap_or(&root).join(target);
                if absent_private(&root, &path) {
                    continue;
                }
                checked += 1;
                if !path.exists() {
                    let rel = file.strip_prefix(&root).unwrap_or(file);
                    broken.push(format!("{}:{}: {target}", rel.display(), number + 1));
                }
            }
        }
    }
    let mut lines = vec![format!(
        "docs: {checked} links in {} files",
        files.iter().filter(|f| !archived(&root, f)).count()
    )];
    lines.extend(broken.iter().cloned());
    (lines, !broken.is_empty())
}
