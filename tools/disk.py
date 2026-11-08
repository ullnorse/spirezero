#!/usr/bin/env python3
"""Disk report and garbage collector for the checkouts' target/ directories and the shared caches (see tools/disk.sh)."""

import argparse
import fcntl
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

CACHE_HOME = Path.home() / ".cache"
TRACES = Path(cache_dir("referee-traces"))
# Directly in a profile directory (target/debug): not artifacts, never trimmed.
KEEP_TOP = {"CACHEDIR.TAG", ".rustc_info.json"}


def sh(*cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False).stdout


def size(path):
    """Disk usage of `path` in bytes (0 if it is gone)."""
    if not os.path.lexists(path):
        return 0
    out = sh("du", "-sk", "--", str(path)).split()
    return int(out[0]) * 1024 if out else 0


def human(n):
    for unit in ("B", "K", "M", "G"):
        if n < 1024 or unit == "G":
            return f"{n:.0f}{unit}" if unit in "BK" else f"{n:.1f}{unit}"
        n /= 1024
    return ""


class Worktree:
    def __init__(self, path, branch, prunable, is_main):
        self.path = Path(path)
        self.branch = branch
        self.prunable = prunable or not self.path.is_dir()
        self.is_main = is_main
        self.target = self.path / "target"
        self.merged = False


def worktrees(repo):
    """The worktrees `git worktree list` knows, the primary checkout first."""
    out = sh("git", "worktree", "list", "--porcelain", cwd=repo)
    found = []
    for block in out.strip().split("\n\n"):
        fields = {}
        for line in block.splitlines():
            key, _, value = line.partition(" ")
            fields[key] = value
        if "worktree" not in fields:
            continue
        branch = fields.get("branch", "").removeprefix("refs/heads/")
        found.append(Worktree(fields["worktree"], branch or None, "prunable" in fields, not found))
    main = found[0]
    for wt in found[1:]:
        # A branch whose tip main contains. A task branch just created from main also is one: `busy` protects it.
        wt.merged = bool(wt.branch) and subprocess.run(
            ["git", "merge-base", "--is-ancestor", wt.branch, "main"], cwd=main.path, check=False
        ).returncode == 0
    return found


def profile_dirs(target):
    """The directories under `target` that cargo locks (`<profile>/.cargo-lock`), at most two levels deep."""
    found = []
    for lock in sorted(target.glob(".cargo-lock")) + sorted(target.glob("*/.cargo-lock")) + sorted(target.glob("*/*/.cargo-lock")):
        found.append(lock.parent)
    return found


def fresh(path, days):
    """Whether anything under `path` was modified less than `days` days ago."""
    return bool(sh("find", str(path), "-mtime", f"-{days}", "-print", "-quit").strip())


def busy(wt, min_age_minutes):
    """Why `wt` looks in use (a reason), or None: a process runs in it or from its target/, a cargo build holds a profile's
    lock, or its target/ was written in the last `min_age_minutes` (a worker between two commands)."""
    root = str(wt.path.resolve())
    me = os.getpid()
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit() or int(proc.name) == me:
            continue
        for link in ("cwd", "exe"):
            try:
                where = os.readlink(proc / link)
            except OSError:
                continue
            if where == root or where.startswith(root + "/"):
                return f"process {proc.name} ({link})"
    if not wt.target.is_dir():
        return None
    for profile in profile_dirs(wt.target):
        with (profile / ".cargo-lock").open("a") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                return f"cargo build in {profile.name}"
    if sh("find", str(wt.target), "-mmin", f"-{min_age_minutes}", "-print", "-quit").strip():
        return f"target/ written in the last {min_age_minutes} min"
    return None


class Collector:
    def __init__(self, dry_run):
        self.dry_run = dry_run
        self.freed = 0

    def remove(self, path, label=None):
        path = Path(path)
        freed = size(path)
        self.freed += freed
        if not self.dry_run:
            if path.is_dir() and not path.is_symlink():
                shutil.rmtree(path, ignore_errors=True)
            else:
                path.unlink(missing_ok=True)
        return freed

    def trim_profile(self, profile, days):
        """Old artifacts in one profile directory, while holding cargo's lock on it (a build that starts meanwhile waits)."""
        with (profile / ".cargo-lock").open("a") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                return 0, "locked"
            before = self.freed
            cutoff = time.time() - days * 86400
            # rustc's incremental sessions: the newest of a crate is its cache, older than `days` it is a full rebuild at worst.
            for session in sorted(profile.glob("incremental/*/s-*")):
                if session.is_dir() and session.stat().st_mtime < cutoff:
                    self.remove(session)
                    self.remove(session.with_name(session.name.rsplit("-", 1)[0] + ".lock"))
            # Artifacts (also the binaries linked into the profile's top level) neither read nor written for `days` days;
            # cargo rebuilds what is missing. `deps` and `examples` hold the test binaries too, including those of test
            # files that were renamed or merged since.
            for folder in (profile, profile / "deps", profile / "examples"):
                if not folder.is_dir():
                    continue
                for entry in sorted(folder.iterdir()):
                    if entry.name in KEEP_TOP or entry.name.startswith(".") or not entry.is_file():
                        continue
                    st = entry.lstat()
                    if max(st.st_atime, st.st_mtime) < cutoff:
                        self.remove(entry)
            return self.freed - before, None

    def trim_scratch(self, target, days):
        """Work directories of old diff and regress runs (target/diff/*, target/regress/work/*)."""
        before = self.freed
        for parent in (target / "diff", target / "regress" / "work"):
            if parent.is_dir():
                for entry in sorted(parent.iterdir()):
                    if not fresh(entry, days):
                        self.remove(entry)
        return self.freed - before

    def trim_traces(self, days):
        """Traces of the shared referee store (and their `.fail` marks) not read for `days` days."""
        if not TRACES.is_dir():
            return 0
        before = self.freed
        cutoff = time.time() - days * 86400
        now = time.time()
        for entry in os.scandir(TRACES):
            if entry.name.endswith((".jsonl", ".jsonl.gz")) and entry.stat().st_atime < cutoff:
                base = entry.name.split(".")[0]
                self.remove(entry.path)
                self.remove(TRACES / f"{base}.fail")
        # A `.fail` is written just before its trace: only a day-old one without a trace is an orphan.
        for entry in os.scandir(TRACES):
            if entry.name.endswith(".fail") and now - entry.stat().st_mtime > 86400:
                base = entry.name.removesuffix(".fail")
                if not ((TRACES / f"{base}.jsonl").exists() or (TRACES / f"{base}.jsonl.gz").exists()):
                    self.remove(entry.path)
            elif entry.name.startswith(".tmp.") and now - entry.stat().st_mtime > 86400:
                self.remove(entry.path)
        return self.freed - before


def state(wt):
    if wt.prunable:
        return "missing"
    if wt.is_main:
        return "main"
    if wt.branch is None:
        return "detached"
    return "merged" if wt.merged else "live"


def report(wts):
    rows = []
    total = 0
    for wt in wts:
        target = size(wt.target) if wt.target.is_dir() else 0
        total += target
        parts = {name: size(wt.target / name) for name in ("debug", "regress", "fast")}
        other = target - sum(parts.values())
        rows.append((wt, target, parts, other))
    width = max(len(str(wt.path)) for wt in wts)
    print(f"{'checkout':<{width}}  {'branch':<14} {'state':<8} {'target':>7} {'debug':>7} {'regress':>7} {'fast':>7} {'other':>7}")
    for wt, target, parts, other in rows:
        print(
            f"{wt.path!s:<{width}}  {(wt.branch or '-'):<14} {state(wt):<8} {human(target):>7} {human(parts['debug']):>7} "
            f"{human(parts['regress']):>7} {human(parts['fast']):>7} {human(other):>7}"
        )
    print(f"{'target/ of all checkouts':<{width}}  {'':<14} {'':<8} {human(total):>7}")
    print()
    builds = wts[0].path / "referee" / "builds"
    print(f"referee builds of the main checkout (never collected): {human(size(builds))}")
    caches = sorted([*CACHE_HOME.glob("spirezero-*"), *CACHE_HOME.glob("spirebot-*")])
    cache_total = 0
    for cache in caches:
        n = size(cache)
        cache_total += n
        print(f"  {str(cache).replace(str(Path.home()), '~'):<44} {human(n):>7}")
    print(f"  {'shared caches':<44} {human(cache_total):>7}")
    usage = shutil.disk_usage(CACHE_HOME)
    print(f"disk: {human(usage.used)} used of {human(usage.total)}, {human(usage.free)} free")
    return total


def gc(wts, args):
    col = Collector(args.dry_run)
    verb = "would remove" if args.dry_run else "removed"
    if any(wt.prunable for wt in wts) and not args.dry_run:
        sh("git", "worktree", "prune", cwd=wts[0].path)
    for wt in wts:
        if wt.prunable:
            print(f"{wt.path}: worktree is gone, {'prune it' if args.dry_run else 'pruned'}")
    for wt in wts:
        if wt.prunable or not wt.target.is_dir() or wt.target.is_symlink():
            continue
        why = busy(wt, args.min_age) if not wt.is_main else None
        if wt.merged and why is None:
            freed = col.remove(wt.target)
            print(f"{wt.path}: branch {wt.branch} is merged into main, {verb} target/ ({human(freed)})")
            continue
        if wt.merged:
            print(f"{wt.path}: branch {wt.branch} is merged but the checkout looks in use ({why}), trimming only")
        trimmed = 0
        for profile in profile_dirs(wt.target):
            freed, skipped = col.trim_profile(profile, args.days)
            if skipped:
                print(f"{wt.path}: {profile.relative_to(wt.path)} is being built, skipped")
            trimmed += freed
        scratch = col.trim_scratch(wt.target, args.days)
        print(f"{wt.path}: {verb} {human(trimmed)} of artifacts and {human(scratch)} of scratch older than {args.days} days")
    traces = col.trim_traces(args.trace_days)
    print(f"{TRACES}: {verb} {human(traces)} of traces not read for {args.trace_days} days")
    print(f"{verb} {human(col.freed)} in all")


def main():
    parser = argparse.ArgumentParser(prog="tools/disk.sh", description=__doc__)
    parser.add_argument("--gc", action="store_true", help="collect garbage (safe while workers run), then report")
    parser.add_argument("-n", "--dry-run", action="store_true", help="with --gc: only say what would go")
    parser.add_argument("--days", type=int, default=3, help="artifacts, incremental sessions and scratch untouched this long go (default 3)")
    parser.add_argument("--min-age", type=int, default=120, help="minutes since a merged branch's target/ was written before it goes (default 120)")
    parser.add_argument("--trace-days", type=int, default=30, help="referee traces not read this long go (default 30)")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    wts = worktrees(repo)
    if args.gc:
        gc(wts, args)
        print()
        wts = worktrees(repo)
    report(wts)


if __name__ == "__main__":
    sys.exit(main())
