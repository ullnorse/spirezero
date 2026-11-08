#!/usr/bin/env bash
# Usage: tools/disk.sh [--gc [-n] [--days N] [--min-age MINUTES] [--trace-days N]]
# Without arguments: a table of the target/ size of every checkout (`git worktree list`, so the primary checkout and each
# worker's worktree) and of the shared caches ~/.cache/spirezero-*, and the free space.
# --gc first collects, in this order, and is safe while other workers build, test or regress:
#   * worktrees whose directory is gone are pruned (`git worktree prune`);
#   * target/ of a worktree whose branch is merged into main is deleted, unless the checkout looks in use (a process with its
#     cwd or binary in it, a cargo build holding a profile's lock, target/ written in the last --min-age minutes, default 120);
#   * in every other checkout (the primary one included): incremental sessions and artifacts (deps/, examples/, the
#     binaries in the profile directory) neither read nor written for --days days (default 3), and diff/regress work
#     directories (target/diff, target/regress/work) untouched that long. A profile is trimmed while holding cargo's own lock on
#     it (target/<profile>/.cargo-lock), so a build that starts meanwhile waits and one in progress skips the profile;
#   * traces in ~/.cache/spirezero-referee-traces not read for --trace-days days (default 30).
# -n shows what --gc would remove. It never touches source files, referee/builds, or any directory outside target/ and the trace store.
exec python3 -I "$(dirname "$0")/disk.py" "$@"
