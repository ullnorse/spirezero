#!/usr/bin/env python3
"""Job list and cache keys for tools/regress.sh.

Usage: regress_plan.py <repo root> <seeds> <referee build name> <max actions> <ascensions...>
       an ascension `mid` (forced fights only) is one more run per encounter and loadout at an ascension from 1 to 9 picked
       by a stable hash of the run's name (see mid_ascension), so every middle ascension is covered by some encounters
       REGRESS_RUNS=1 regress_plan.py ...   (the run loadouts of tools/regress_runs.sh instead)

Prints one line per run, `label ascension encounter loadout|- key character act1` (the character is the
loadout's "character", else CHARACTER, else Necrobinder; act1 is overgrowth); with REGRESS_RUNS=1,
`label ascension policy loadout key character act1` for each crates/sts2-diff/run_loadouts/*.json (its
`"policy"`, default greedy, or a list of policies, one run each, `"ascensions"`, default every one asked for, `"characters"`, default
Necrobinder, and `"acts"`, default overgrowth; the CHARACTERS and ACTS environment variables, space
separated, replace the last two for every loadout), whose engine hash also covers
tools/diff_run.sh and whose content closure is seeded by the loadout's tokens and the starters only
(a run reaches content by random draws, the blind spot below). The key is the first 24 hex
digits of sha256 over
  * the engine hash: every source file the regress binary is built from, as cargo's dep-info lists it
    (target/regress/sts2-diff.d, written by the build regress.sh runs first; a missing one is an error):
    crates/sts2-diff/src, crates/sts2-core/src without its test modules, crates/sts2-core/build.rs and
    crates/sts2-macros, except the files directly in a `content/<kind>/` folder (the entity modules, the
    `mod.rs` registries and `pools.rs`, which every content merge edits) and build.rs's output under
    target/ (generated from those folders); plus every workspace crate's Cargo.toml, the workspace
    Cargo.toml and Cargo.lock, the scripts a job runs (tools/diff.sh, tools/slot.sh, referee/client.sh,
    referee/referee.py, referee/trace_cache.py), the referee build and MAX_ACTIONS;
  * the content hash: the entity modules the run reaches, found by following identifiers (CamelCase
    or snake_case that name another entity module) from the seeds below;
  * the seed count, ascension, encounter and the loadout file's bytes.
Seeds: the loadout's own tokens (ids lower-cased), the encounter, and the character's starter
entities (files whose name ends in `_<character>`, plus the Necrobinder starter relic and cards).
Over-approximating is safe (a spurious re-run). The blind spots, covered by `REGRESS_NO_CACHE=1`
(final_check.sh): a registry or pool edit that changes the behaviour of existing content, and
content reached only through a random pool draw.
"""
import hashlib
import json
import os
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
seeds, referee, max_actions = sys.argv[2], sys.argv[3], sys.argv[4]
ascensions = sys.argv[5:]
core = root / "crates" / "sts2-core" / "src"
ENTITY_DIRS = {"afflictions", "cards", "enchantments", "encounters", "events", "monsters",
               "potions", "powers", "relics"}
STARTER = ["bound_phylactery", "bodyguard", "unleash"]  # + *_necrobinder; the default character


def sha(data):
    return hashlib.sha256(data).hexdigest()


def mid_ascension(name, allowed=range(1, 10)):
    """The middle ascension (1 to 9, within `allowed`) of the forced fight `name`, fixed by a hash of the name alone so the
    run keeps its cache key from one regress to the next; None if none is allowed."""
    allowed = [a for a in allowed if 1 <= a <= 9]
    return allowed[int(sha(name.encode())[:8], 16) % len(allowed)] if allowed else None


def in_kind_dir(path):
    rel = path.relative_to(core / "content")
    return len(rel.parts) == 2 and rel.parts[0] in ENTITY_DIRS


def is_entity(path):
    return in_kind_dir(path) and path.name not in ("mod.rs", "pools.rs")


def snake(name):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name).lower()


entities = {}  # snake name -> [paths]
for path in sorted(core.rglob("*.rs")):
    if "content" in path.relative_to(core).parts[:1] and is_entity(path):
        entities.setdefault(path.stem, []).append(path)
digest = {p: sha(p.read_bytes()) for ps in entities.values() for p in ps}

engine = hashlib.sha256()
def is_registry(path):
    """An entity module, a kind's `mod.rs` registry or `pools.rs` (hashed per run by the content closure)."""
    return path.is_relative_to(core / "content") and in_kind_dir(path)


def dep_info(path):
    """The source files cargo's dep-info file `path` lists (`target: dep dep ...`, spaces escaped as `\\ `)."""
    if not path.is_file():
        sys.exit(f"regress_plan.py: no {path}; build first (cargo build --profile regress -p sts2-diff)")
    text = path.read_text(encoding="utf-8").split("\n", 1)[0]
    deps = re.split(r"(?<!\\) ", text.split(": ", 1)[1].strip())
    return [Path(d.replace("\\ ", " ")) for d in deps if d]


target = (root / "target").resolve()
files = set()
for p in dep_info(root / "target" / "regress" / "sts2-diff.d"):
    p = p.resolve()
    if p.is_relative_to(target):  # build.rs's output, generated from the content folders and build.rs (hashed)
        continue
    if p.is_dir():  # a folder build.rs watches (`rerun-if-changed`); its files are hashed on their own
        continue
    if not p.is_relative_to(root.resolve()):
        sys.exit(f"regress_plan.py: dep-info lists {p}, outside {root}")
    p = root / p.relative_to(root.resolve())
    if is_registry(p):
        continue
    files.add(p)
members = re.search(r"members\s*=\s*\[([^\]]*)\]", (root / "Cargo.toml").read_text(encoding="utf-8"))
if not members:
    sys.exit("regress_plan.py: no workspace members in Cargo.toml")
files |= {root / m / "Cargo.toml" for m in re.findall(r'"([^"]+)"', members.group(1))}
files |= {root / "Cargo.toml", root / "Cargo.lock", root / "tools" / "diff.sh", root / "tools" / "slot.sh",
          root / "referee" / "client.sh", root / "referee" / "referee.py", root / "referee" / "trace_cache.py"}
for p in sorted(files):
    engine.update(f"{p.relative_to(root)} {sha(p.read_bytes())}\n".encode())
engine.update(f"{referee} {max_actions}\n".encode())
engine = engine.hexdigest()

TOKEN = re.compile(r"[A-Za-z][A-Za-z0-9_]*")
tokens_of = {}


def names_in(path):
    """Entity module names a file mentions."""
    if path not in tokens_of:
        found = set()
        for token in TOKEN.findall(path.read_text(encoding="utf-8")):
            for candidate in {token, token.lower(), snake(token)}:
                for variant in (candidate, candidate.removesuffix("_power")):
                    if variant in entities:
                        found.add(variant)
        tokens_of[path] = found
    return tokens_of[path]


def closure(seed_names):
    seen, todo = set(), list(seed_names)
    while todo:
        name = todo.pop()
        if name in seen or name not in entities:
            continue
        seen.add(name)
        for path in entities[name]:
            todo.extend(names_in(path))
    return seen


def content_hash(loadout, encounter, character=None):
    text = loadout.read_text(encoding="utf-8") if loadout else ""
    data = json.loads(text) if text else {}
    character = str(character or data.get("character", "Necrobinder")).lower()
    seeds_ = {t.lower() for t in TOKEN.findall(text)} | {snake(encounter), encounter.lower()}
    seeds_ |= set(STARTER) | {n for n in entities if n.endswith("_" + character)}
    h = hashlib.sha256()
    for name in sorted(closure(seeds_)):
        for path in entities[name]:
            h.update(f"{path.relative_to(root)} {digest[path]}\n".encode())
    return h.hexdigest()


def job(label, asc, encounter, loadout, engine=engine, character=None, act1=None):
    parts = [engine, content_hash(loadout, encounter, character), seeds, asc, encounter,
             sha(loadout.read_bytes()) if loadout else "-"]
    # The default run axes leave the key (and the label) as they were before the axes existed.
    if character not in (None, "Necrobinder") or act1 not in (None, "overgrowth"):
        parts += [character, act1]
    key = sha(" ".join(map(str, parts)).encode())[:24]
    print(label, asc, encounter, loadout or "-", key, character or "Necrobinder", act1 or "overgrowth")


# The acts a whole run can be played in. TODO(T2/T3): add "hive" and "glory" once diff_run.sh plays multi-act runs.
RUN_ACTS = {"overgrowth", "underdocks"}

if os.environ.get("REGRESS_RUNS"):
    engine_runs = sha(f"{engine} {sha((root / 'tools' / 'diff_run.sh').read_bytes())}".encode())
    for path in sorted((root / "crates" / "sts2-diff" / "run_loadouts").glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        policy = data.get("policy", "greedy")
        policies = policy if isinstance(policy, list) else [policy]
        characters = os.environ.get("CHARACTERS", "").split() or data.get("characters", ["Necrobinder"])
        acts = os.environ.get("ACTS", "").split() or data.get("acts", ["overgrowth"])
        for skipped in sorted(set(acts) - RUN_ACTS):
            print(f"regress_runs: {path.stem}: act {skipped} skipped (no multi-act runs yet, T2/T3)", file=sys.stderr)
        acts = [a for a in acts if a in RUN_ACTS]
        for asc in ascensions:
            if int(asc) not in data.get("ascensions", [int(asc)]):
                continue
            for character in characters:
                for act1 in acts:
                    suffix = ("" if character == "Necrobinder" else f"-{character.lower()}") + \
                        ("" if act1 == "overgrowth" else f"-{act1}")
                    for policy in policies:
                        job(f"{path.stem}-{policy}-a{asc}{suffix}", asc, policy, path, engine_runs, character, act1)
    sys.exit(0)

mid = "mid" in ascensions
ascensions = [a for a in ascensions if a != "mid"]
loadouts = sorted((root / "crates" / "sts2-diff" / "loadouts").glob("*.json"))
# CHARACTER plays every encounter as that character; a loadout's own "character" wins.
default_character = os.environ.get("CHARACTER") or "Necrobinder"
character_suffix = "" if default_character == "Necrobinder" else f"-{default_character.lower()}"
for asc in ascensions + (["mid"] if mid else []):
    for path in sorted((core / "content" / "encounters").glob("*.rs")):
        if path.stem != "mod":
            camel = re.sub(r"(^|_)([a-z])", lambda m: m.group(2).upper(), path.stem)
            run_asc = mid_ascension(camel + character_suffix) if asc == "mid" else asc
            job(f"{camel}-a{run_asc}{character_suffix}", run_asc, camel, None, character=default_character)
    for path in loadouts:
        data = json.loads(path.read_text(encoding="utf-8"))
        if asc != "mid" and int(asc) not in data.get("ascensions", [int(asc)]):
            continue
        character = data.get("character") or default_character
        suffix = "" if "character" in data else character_suffix
        for encounter in data.get("encounters", ["FuzzyWurmCrawlerWeak", "SlimesWeak"]):
            run_asc = mid_ascension(f"{path.stem}-{encounter}{suffix}", data.get("ascensions", range(1, 10))) if asc == "mid" else asc
            if run_asc is not None:
                job(f"{path.stem}-{encounter}-a{run_asc}{suffix}", run_asc, encounter, path, character=character)
