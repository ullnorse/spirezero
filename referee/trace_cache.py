"""Content-addressed store of referee traces (T12): the game runs only for an input it has never seen.

The referee is deterministic: the same build, the same `referee.py` and the same actions file give the same
trace (referee/README.md, "Same input twice"). `key` hashes everything a trace depends on:
  * the build: its `builds/<hash>` name (setup.sh, tracer/, patcher/) plus size and mtime of `sts2.dll` and
    `Sts2Headless.dll`, so a build directory rebuilt from other game DLLs under the same name is a new key;
  * `referee.py` (the protocol driving, the snapshot format; this module does not change a trace);
  * every `STS2_*` environment variable the game process inherits (`STS2_GAME_DIR`, `STS2_LIB` pick the DLLs,
    `STS2_SAVE_SCHEMA_VERSION` and the `STS2_SOLVER_*` ones are read by sts2-cli). `STS2_ACT1` and `STS2_REAL_*`
    are not inputs: `Game` drops them and `reset` sets them from the actions file. `STS2_REFEREE` is the build
    above and `STS2_GAME` is only read by setup.sh. `DOTNET_ROOT` picks the runtime;
  * the seed and the actions file as canonical JSON (keys sorted, no whitespace), which holds every other input:
    character, ascension, encounter, neow, loadout, mode, act1, the `real_*` flags and the actions.
`MAX_ACTIONS` and the policy only shape the actions on the engine side, so they are inside the file already.

Call sites (referee.py): `run`, and `serve`/`client` per job through `cached_job`. The client looks up before it
connects, so a hit never reaches the slot daemon (tools/slot.sh), and stores what the daemon wrote; the daemon itself
does not use the store.

Layout: `<store>/<key>.jsonl.gz` is the trace, gzip level 6 (a trace is ~175 KB of JSON lines and shrinks to ~2.4%; reading one
costs ~0.06 ms more). Entries written before compression, `<key>.jsonl` in plain text, are still read (the plain file
first, so `compress` below can delete it after writing the `.gz` without a reader missing either). A run that failed (a rejected action, an unreachable combat) also has
`<key>.fail` with the reason; it is written before the trace, so a reader that sees the trace sees the mark too.
Failures of the process itself (it died, `reset` failed) are not stored, and neither is any failure of a reused
process (`serve`, `client`): only `run`, a fresh process per seed, stores failures. A reused process is not always
byte-identical to a fresh one: `p12-entropy` SlimesWeak a0, D1S0 then D1S1 in one `serve` fails D1S1 with "could not
reach combat", which alone passes (2026-10-18, after T11). A failure from a reused process is replayed next time. Writes go to a temp file in the store
and are renamed into place, so concurrent workers never see a partial file; the last writer wins with identical
bytes. The store is `~/.cache/spirezero-referee-traces` (the old `spirebot-referee-traces` while only it exists) (`REFEREE_TRACE_CACHE` elsewhere); `REFEREE_NO_TRACE_CACHE=1`
bypasses it. Nothing is pruned: `find <store> -atime +30 -delete` is safe at any time. `python3 referee/trace_cache.py compress`
converts the plain entries in place (safe while other workers run).
`REFEREE_STATS=<file>` appends one line per event (`hit`, `miss`, `game` for a game process started).
"""

import gzip
import hashlib
import json
import os
import sys
import tempfile
from pathlib import Path


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

STORE = Path(os.environ.get("REFEREE_TRACE_CACHE", cache_dir("referee-traces")))
FORMAT = "1"  # bump to orphan every stored trace (compression does not change a key)
GZIP_LEVEL = 6
# Set per run by referee.py from the actions file, or not read by the game process.
NOT_INPUTS = {"STS2_ACT1", "STS2_REAL_SHOP", "STS2_REAL_CAULDRON", "STS2_REAL_BELL", "STS2_REFEREE", "STS2_GAME"}
# Failures of the process, not of the input: never stored.
TRANSIENT = ("game process exited unexpectedly", "reset failed", "referee daemon died", "could not start the referee daemon",
             "the referee daemon at")
_REFEREE_PY = hashlib.sha256((Path(__file__).resolve().parent / "referee.py").read_bytes()).hexdigest()


def key(seed, spec, build, dotnet):
    """The store key of `seed` played from `spec` (the parsed actions file) on `build`, or None when bypassed."""
    if os.environ.get("REFEREE_NO_TRACE_CACHE"):
        return None
    dlls = build / "sts2-cli" / "src" / "Sts2Headless" / "bin" / "Debug" / "net9.0"
    stats = {}
    for dll in (dlls / "Sts2Headless.dll", dlls / "sts2.dll", build / "sts2-cli" / "lib" / "sts2.dll"):
        if dll.exists():
            st = dll.stat()
            stats[str(dll.relative_to(build))] = [st.st_size, st.st_mtime_ns]
    inputs = {
        "format": FORMAT,
        "build": build.name,
        "dlls": stats,
        "referee_py": _REFEREE_PY,
        "env": env_inputs(),
        "dotnet": str(dotnet),
        "seed": seed,
        "spec": spec,
    }
    return hashlib.sha256(json.dumps(inputs, sort_keys=True, separators=(",", ":")).encode()).hexdigest()[:32]


def env_inputs():
    """The environment variables the game process reads that are not set per run (see the module docs)."""
    return {k: v for k, v in sorted(os.environ.items()) if k.startswith("STS2_") and k not in NOT_INPUTS and v}


def answer(k, out, run):
    """Write the trace stored under `k` to `out` and return its status; on a miss, `run()` (which writes `out` and
    returns `ok` or `fail <reason>`) and store the result if it passed. `run` here is a reused game process, whose
    failures are not stored (see the module docs)."""
    if (hit := lookup(k)) is not None:
        out.write_text(hit[0])
        return hit[1]
    status = run()
    if k is not None and status == "ok" and out.exists():
        store(k, out.read_text(), status)
    return status


def lookup(k):
    """`(trace text, status)` stored under `k` (status `ok` or `fail <reason>`), or None (and `miss` noted)."""
    if k is None:
        return None
    trace = _read(k)
    if trace is None:
        note("miss")
        return None
    fail = STORE / f"{k}.fail"
    status = f"fail {fail.read_text()}" if fail.exists() else "ok"
    note("hit")
    return trace, status


def _read(k):
    """The trace text stored under `k` (compressed or plain), or None."""
    try:
        return (STORE / f"{k}.jsonl").read_text()
    except FileNotFoundError:
        pass
    try:
        return gzip.decompress((STORE / f"{k}.jsonl.gz").read_bytes()).decode()
    except FileNotFoundError:
        return None


def _compressed(text):
    """`text` as gzip bytes; the header time is 0, so the same trace is the same bytes for every writer."""
    return gzip.compress(text.encode(), GZIP_LEVEL, mtime=0)


def store(k, trace, status):
    """Store a finished run: `trace` is its text, `status` `ok` or `fail <reason>`."""
    if k is None:
        return
    reason = status.removeprefix("fail ") if status != "ok" else None
    if reason is not None and reason.startswith(TRANSIENT):
        return
    STORE.mkdir(parents=True, exist_ok=True)
    if reason is not None:
        _write(STORE / f"{k}.fail", reason.encode())
    _write(STORE / f"{k}.jsonl.gz", _compressed(trace))


def _write(path, data):
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix=".tmp.")
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.replace(tmp, path)
    except BaseException:
        Path(tmp).unlink(missing_ok=True)
        raise


def note(event):
    """Append `event` to `$REFEREE_STATS` if set (O_APPEND lines are atomic between processes).

    Best effort: a slot daemon (referee/client.sh) keeps the environment of the job that started it, so the file can be
    in a checkout that was deleted since; the counts are only a report, so that must not fail the trace."""
    path = os.environ.get("REFEREE_STATS")
    if path:
        try:
            with open(path, "a") as f:
                f.write(event + "\n")
        except OSError:
            pass


def compress_store():
    """Convert every plain `<key>.jsonl` to `<key>.jsonl.gz`; returns (files converted, bytes before, bytes after).

    Safe while other workers run: the `.gz` is in place (rename) before the plain file goes, and `lookup` reads the plain
    file first, so a reader gets one or the other."""
    done = before = after = 0
    for plain in sorted(STORE.glob("*.jsonl")):
        try:
            text = plain.read_text()
        except FileNotFoundError:  # another compress got there first
            continue
        data = _compressed(text)
        _write(plain.with_name(plain.name + ".gz"), data)
        before += plain.stat().st_size
        after += len(data)
        plain.unlink(missing_ok=True)
        done += 1
    return done, before, after


if __name__ == "__main__":
    if sys.argv[1:] != ["compress"]:
        sys.exit("usage: trace_cache.py compress   (convert the store's plain traces to gzip)")
    n, b, a = compress_store()
    print(f"{n} traces, {b / 1e6:.1f} MB -> {a / 1e6:.1f} MB")
