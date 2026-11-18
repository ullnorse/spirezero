"""The ctypes binding of `crates/sts2-ffi` (header: `crates/sts2-ffi/include/sts2.h`).

`load()` finds the library, declares every signature and runs the build checks: the ABI
version must be this module's, and the library's build id (`sts2_build_id()`, the hash
`crates/sts2-ffi/build.rs` computes over the sources it was built from) must be the hash of this
checkout's sources, so a trainer never runs a library that is older or newer than its code.
Build the library with `cargo build --profile ffi -p sts2-ffi` (output `target/ffi/`).
"""

import ctypes
import hashlib
import os
import pathlib
import sys

import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[2]
"""The workspace root."""

ABI_VERSION = 15

COMPATIBLE_ABIS = (6, 7, 8, 9, 10, 11, 12, 13, 14, 15)
"""ABIs this binding loads. ABI 7 only added the practice calls (`PRACTICE_SYMBOLS`): an ABI-6
library (a lane's frozen copy) runs everything else the same, and `has_practice` is False.
ABI 8 appended `RunConfig.obs_v2` and `RunLayout.v2_at` (the obs v2 section, RLN16): an older
library ignores the one and leaves the other 0, so everything with `obs_v2` off is the same.
ABI 9 appended the facing and the powers' shown numbers to the v2 section (RLN21), ABI 10 the
plan column (RLN22): the rows grew at their end each time; the encoder's v2 path needs the
latest (`encode.check_layout`). ABI 11 only added the content stats (`CONTENT_STATS_SYMBOLS`,
RLN29): an ABI-10 library runs everything else the same, and `has_content_stats` is False.
ABI 12 appended `RunConfig.obs_full` and `RunLayout.full_at` (the full-view section, RLN30): an
older library ignores the one and leaves the other 0; its rows end before that section.
ABI 13 (RLN32) grew the run space from 355 to 361 indices (discard the potion in belt slot s, after
`PROCEED`) and appended `RunConfig.discard_potion` and `plan_depth`; the encoder reads the 361-index
space, so it needs ABI 13 (`has_discard_potion`).
ABI 14 appended `RunConfig.obs_events` and `RunLayout.events_at` (the event section, RLN33): an
older library ignores the one and leaves the other 0; its rows end before that section.
ABI 15 (RLN43) only added the combat gym (`GYM_SYMBOLS`, run end 9 `gym_end`, cell codes 7-15):
an ABI-14 library runs everything else the same, and `has_gym` is False."""

GYM_SYMBOLS = ("sts2_run_envs_load_gym", "sts2_run_envs_set_gym", "sts2_run_envs_gym_stats",
               "sts2_human_check")

PRACTICE_SYMBOLS = (
    "sts2_run_envs_set_practice", "sts2_run_envs_practice_stats", "sts2_run_envs_final_mask_ptr",
    "sts2_run_envs_final_priv_ptr", "sts2_run_envs_cell_ptr", "sts2_run_envs_ended_cell_ptr",
)

CONTENT_STATS_SYMBOLS = ("sts2_content_stats", "sts2_content_stats_columns")
COPY_SYMBOLS = ("sts2_run_envs_copy",)  # RLN34: missing from frozen libraries before RLN27
RECORD_SYMBOLS = ("sts2_run_envs_start_record",)  # NAZ1: optional, ABI unchanged

# The hashed sources: the same list as `crates/sts2-ffi/build.rs`.
_CRATES = ("sts2-core", "sts2-macros", "sts2-rl", "sts2-ffi")
_ENTRIES = ("Cargo.toml", "build.rs", "src", "vocab")

# Where `load()` looks, in order, under `target/`.
_PROFILES = ("ffi", "release", "debug")

STS2_OK = 0
STS2_ERR_NULL = -1
STS2_ERR_LENGTH = -2
STS2_ERR_ILLEGAL = -3
STS2_ERR_ARGUMENT = -4
STS2_ERR_PANIC = -5
STS2_ERR_POISONED = -6


class StaleBuild(RuntimeError):
    """The library was not built from this checkout's sources (or for another ABI)."""


class Sts2Error(RuntimeError):
    """A call into the library failed; `code` is its `STS2_ERR_*`."""

    def __init__(self, code, message):
        super().__init__(f"{message} (code {code})")
        self.code = code


class StepStats(ctypes.Structure):
    _fields_ = [
        ("actions", ctypes.c_uint64),
        ("unmapped", ctypes.c_uint64),
        ("ended", ctypes.c_uint32),
        ("won", ctypes.c_uint32),
        ("capped", ctypes.c_uint32),
        ("reset", ctypes.c_uint32),
    ]


class Layout(ctypes.Structure):
    _fields_ = [
        (name, ctypes.c_uint64)
        for name in (
            "obs_bytes",
            "static_bytes",
            "mask_words",
            "actions",
            "creatures_at",
            "creature_bytes",
            "cards_at",
            "card_bytes",
            "hand_at",
            "intents_at",
            "intent_bytes",
            "choice_at",
        )
    ]


class RunConfig(ctypes.Structure):
    """`Sts2RunConfig`: the run env's seeds and what it plays."""

    _fields_ = [
        ("seed_kind", ctypes.c_uint32),
        ("cycle", ctypes.c_uint32),
        ("seeds", ctypes.POINTER(ctypes.c_uint64)),
        ("seeds_len", ctypes.c_size_t),
        ("start", ctypes.c_uint64),
        ("end", ctypes.c_uint64),
        ("master", ctypes.c_uint64),
        ("character", ctypes.c_int32),
        ("ascension", ctypes.c_uint32),
        ("combat", ctypes.c_uint32),
        ("intents", ctypes.c_uint32),
        # The end-to-end options (`sts2_rl::RunOptions`).
        ("dedupe", ctypes.c_uint32),
        ("turn_cap", ctypes.c_uint32),
        ("decision_cap", ctypes.c_uint32),
        ("events", ctypes.c_uint32),
        ("counters", ctypes.c_uint32),
        ("priv_obs", ctypes.c_uint32),
        ("obs_v2", ctypes.c_uint32),  # ABI 8
        ("obs_full", ctypes.c_uint32),  # ABI 12
        ("discard_potion", ctypes.c_uint32),  # ABI 13
        ("plan_depth", ctypes.c_uint32),  # ABI 13
        ("obs_events", ctypes.c_uint32),  # ABI 14
    ]


class RunStepStats(ctypes.Structure):
    _fields_ = [
        ("actions", ctypes.c_uint64),
        ("decisions", ctypes.c_uint64),
        ("auto", ctypes.c_uint64),
        ("resolved", ctypes.c_uint64),
        ("refused", ctypes.c_uint64),
        ("unmapped", ctypes.c_uint64),
        ("ended", ctypes.c_uint32),
        ("won", ctypes.c_uint32),
        ("capped", ctypes.c_uint32),
        ("stuck", ctypes.c_uint32),
        ("reset", ctypes.c_uint32),
        ("fight_capped", ctypes.c_uint32),
        ("decision_capped", ctypes.c_uint32),
        ("merged", ctypes.c_uint64),
        ("unmapped_classes", ctypes.c_uint64),
        ("auto_class", ctypes.c_uint64),
        ("unmapped_decisions", ctypes.c_uint64),
        ("saturated", ctypes.c_uint64 * 8),
    ]


SATURATIONS = ("any", "cards", "deck", "creatures", "powers", "choice", "relics", "map")
"""The kinds of `RunStepStats.saturated`, in order (`sts2_rl::SATURATIONS`)."""


class RunEvents(ctypes.Structure):
    """`Sts2RunEvents`: what one step did to an env's episode."""

    _fields_ = [
        ("floors", ctypes.c_uint8),
        ("bosses", ctypes.c_uint8),
        ("boss_acts", ctypes.c_uint8),
        ("won", ctypes.c_uint8),
    ]


CELL_NAMES = ("ELITE_ACT1", "ELITE_ACT2", "ELITE_ACT3", "BOSS_ACT1", "BOSS_ACT2", "BOSS_ACT3")
"""The practice cells by index (`sts2_rl::practice::CELL_NAMES`); a cell code is index + 1."""

GYM_CELL_NAMES = tuple(f"GYM_{k}_ACT{a}" for k in ("ELITE", "BOSS", "HALL") for a in (1, 2, 3))
"""The gym cells by index (`sts2_rl::GYM_CELL_NAMES`, RLN43); a gym cell code is index + 7."""
GYM_CODE = 7


def cell_name(code):
    """The name of a cell code (0 a normal run, 1-6 practice, 7-15 gym)."""
    if code == 0:
        return "RUN"
    return CELL_NAMES[code - 1] if code < GYM_CODE else GYM_CELL_NAMES[code - GYM_CODE]


class GymConfig(ctypes.Structure):
    """`Sts2GymConfig` (ABI 15)."""

    _fields_ = [
        ("first", ctypes.c_uint64),
        ("slots", ctypes.c_uint64),
        ("weights", ctypes.c_uint32 * 9),
        ("variants", ctypes.c_uint32),
        ("hall_hp_pct", ctypes.c_uint32),
        ("sequential", ctypes.c_uint32),
        ("seed", ctypes.c_uint64),
    ]


class GymStats(ctypes.Structure):
    """`Sts2GymStats` (ABI 15): per gym cell."""

    _fields_ = [(name, ctypes.c_uint64 * 9) for name in ("loaded", "wanted", "started", "empty")]


class PracticeConfig(ctypes.Structure):
    """`Sts2PracticeConfig` (ABI 7)."""

    _fields_ = [
        ("slots", ctypes.c_uint64),
        ("weights", ctypes.c_uint32 * 6),
        ("pool", ctypes.c_uint32),
        ("max_uses", ctypes.c_uint32),
        ("min_active", ctypes.c_uint32),
        ("capture", ctypes.c_uint32),
        ("seed", ctypes.c_uint64),
    ]


class PracticeStats(ctypes.Structure):
    """`Sts2PracticeStats` (ABI 7): per cell."""

    _fields_ = [(name, ctypes.c_uint64 * 6) for name in (
        "pool", "captured", "evicted", "used_up", "wanted", "started", "fell_back", "to_run",
        "bad_start")]


class RunLayout(ctypes.Structure):
    _fields_ = [
        (name, ctypes.c_uint64)
        for name in (
            "obs_bytes",
            "priv_bytes",
            "mask_words",
            "actions",
            "run_base",
            "deck_at",
            "relics_at",
            "belt_at",
            "map_at",
            "ancient_at",
            "event_at",
            "rewards_at",
            "select_at",
            "bundles_at",
            "rest_at",
            "treasure_at",
            "shop_at",
            "combat_at",
            "combat_static_at",
            "v2_at",  # ABI 8 (0 from an older library)
            "full_at",  # ABI 12 (0 from an older library)
            "events_at",  # ABI 14 (0 from an older library)
        )
    ]


def source_hash(root=ROOT):
    """The build id a library built from the sources under `root` has (`build.rs`'s hash)."""
    root = pathlib.Path(root)
    paths = [root / "Cargo.toml", root / "Cargo.lock"]
    for crate in _CRATES:
        paths.extend(root / "crates" / crate / entry for entry in _ENTRIES)
    files = []
    for path in paths:
        if path.is_dir():
            files.extend(p for p in path.rglob("*") if p.is_file())
        elif path.is_file():
            files.append(path)
    named = sorted(
        ("/".join(f.relative_to(root).parts).encode(), f) for f in files
    )
    sha = hashlib.sha256()
    for name, path in named:
        contents = path.read_bytes()
        sha.update(name)
        sha.update(b"\0")
        sha.update(len(contents).to_bytes(8, "little"))
        sha.update(contents)
    return sha.hexdigest()[:16]


def library_name():
    if sys.platform == "darwin":
        return "libsts2_ffi.dylib"
    if sys.platform == "win32":
        return "sts2_ffi.dll"
    return "libsts2_ffi.so"


def find_library():
    """`$STS2_FFI_LIB`, else the first of `target/{ffi,release,debug}/` that has the library."""
    explicit = os.environ.get("STS2_FFI_LIB")
    if explicit:
        return pathlib.Path(explicit)
    for profile in _PROFILES:
        path = ROOT / "target" / profile / library_name()
        if path.exists():
            return path
    raise FileNotFoundError(
        f"no {library_name()} under {ROOT / 'target'}: run "
        "`cargo build --profile ffi -p sts2-ffi` (or set STS2_FFI_LIB)"
    )


def _declare(lib):
    c = ctypes
    envs = c.c_void_p
    signatures = {
        "sts2_abi_version": ([], c.c_uint32),
        "sts2_build_id": ([], c.c_char_p),
        "sts2_vocab_hash": ([], c.c_uint64),
        "sts2_last_error": ([], c.c_char_p),
        "sts2_layout": ([c.POINTER(Layout)], c.c_int32),
        "sts2_envs_new": ([c.c_size_t, c.c_char_p, c.c_uint64, c.c_size_t], envs),
        "sts2_envs_new_from_corpus": (
            [c.c_size_t, c.c_char_p, c.c_size_t, c.c_uint64, c.c_size_t],
            envs,
        ),
        "sts2_envs_free": ([envs], None),
        "sts2_envs_step": ([envs, c.POINTER(c.c_uint16), c.c_size_t], c.c_int32),
        "sts2_envs_reset_all": ([envs], c.c_int32),
        "sts2_envs_stats": ([envs, c.POINTER(StepStats)], c.c_int32),
        "sts2_envs_len": ([envs], c.c_size_t),
        "sts2_envs_obs_bytes": ([envs], c.c_size_t),
        "sts2_envs_obs_ptr": ([envs], c.c_void_p),
        "sts2_envs_static_ptr": ([envs], c.c_void_p),
        "sts2_envs_mask_ptr": ([envs], c.c_void_p),
        "sts2_envs_done_ptr": ([envs], c.c_void_p),
        "sts2_envs_won_ptr": ([envs], c.c_void_p),
        "sts2_envs_final_hp_ptr": ([envs], c.c_void_p),
        "sts2_envs_set_blindfold": ([envs, c.c_uint8, c.c_uint64], c.c_int32),
        "sts2_run_layout": ([c.POINTER(RunLayout)], c.c_int32),
        "sts2_run_envs_new": ([c.c_size_t, c.POINTER(RunConfig), c.c_size_t], envs),
        "sts2_run_envs_free": ([envs], None),
        "sts2_run_envs_step": ([envs, c.POINTER(c.c_uint16), c.c_size_t], c.c_int32),
        "sts2_run_envs_reset_all": ([envs], c.c_int32),
        "sts2_run_envs_restart": ([envs, c.POINTER(c.c_uint64), c.c_size_t, c.c_uint64],
                                  c.c_int32),
        "sts2_run_envs_copy": ([envs, c.POINTER(c.c_uint32), c.POINTER(c.c_uint32),
                                c.POINTER(c.c_uint64), c.c_size_t], c.c_int32),
        "sts2_run_envs_stats": ([envs, c.POINTER(RunStepStats)], c.c_int32),
        "sts2_run_envs_len": ([envs], c.c_size_t),
        "sts2_run_envs_obs_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_final_obs_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_mask_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_done_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_won_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_end_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_floor_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_class_counts_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_events_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_priv_ptr": ([envs], c.c_void_p),
        "sts2_run_action_table": ([], c.c_void_p),
        "sts2_run_envs_set_practice": ([envs, c.POINTER(PracticeConfig)], c.c_int32),
        "sts2_run_envs_practice_stats": ([envs, c.POINTER(PracticeStats)], c.c_int32),
        "sts2_run_envs_final_mask_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_final_priv_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_cell_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_ended_cell_ptr": ([envs], c.c_void_p),
        "sts2_content_stats": ([c.c_uint32, c.POINTER(c.c_size_t)], c.c_void_p),
        "sts2_content_stats_columns": ([], c.c_char_p),
        "sts2_run_envs_load_gym": ([envs, c.c_char_p], c.c_int64),
        "sts2_run_envs_set_gym": ([envs, c.POINTER(GymConfig)], c.c_int32),
        "sts2_run_envs_gym_stats": ([envs, c.POINTER(GymStats)], c.c_int32),
        "sts2_human_check": ([c.c_char_p, c.c_uint64], c.c_int32),
        "sts2_run_envs_start_record": ([envs, c.c_size_t, c.c_char_p, c.c_size_t], c.c_int64),
    }
    for name, (args, result) in signatures.items():
        if (name in PRACTICE_SYMBOLS + CONTENT_STATS_SYMBOLS + COPY_SYMBOLS + GYM_SYMBOLS
                + RECORD_SYMBOLS
                and not hasattr(lib, name)):
            continue  # an ABI-6 library (practice), an ABI 6-10 one (content stats), no copy
        f = getattr(lib, name)
        f.argtypes = args
        f.restype = result
    return lib


def check_build(lib, expected_build_id=None):
    """Raises `StaleBuild` unless `lib` has this module's ABI and was built from
    `expected_build_id`'s sources (default: this checkout's)."""
    abi = lib.sts2_abi_version()
    if abi not in COMPATIBLE_ABIS:
        raise StaleBuild(f"the library has ABI {abi}, this binding {COMPATIBLE_ABIS}")
    built = lib.sts2_build_id().decode()
    # `$STS2_FFI_BUILD_ID` pins a frozen copy of the library (the e2e lane) to its own build.
    expected = expected_build_id or os.environ.get("STS2_FFI_BUILD_ID") or source_hash()
    if built != expected:
        raise StaleBuild(
            f"stale sts2-ffi build: the library was built from sources {built}, this checkout "
            f"is {expected}; rebuild with `cargo build --profile ffi -p sts2-ffi`"
        )


_LOADED = {}


def load(path=None, expected_build_id=None):
    """The library at `path` (default `find_library()`), with its signatures declared and the
    build checks passed."""
    path = pathlib.Path(path) if path else find_library()
    key = str(path.resolve())
    lib = _LOADED.get(key)
    if lib is None:
        lib = _declare(ctypes.CDLL(key))
        check_build(lib, expected_build_id)
        _LOADED[key] = lib
    return lib


def has_practice(lib):
    """Whether `lib` has the practice calls (ABI 7 on)."""
    return lib.sts2_abi_version() >= 7 and all(hasattr(lib, n) for n in PRACTICE_SYMBOLS)


def has_gym(lib):
    """Whether `lib` has the combat gym (ABI 15 on, RLN43)."""
    return lib.sts2_abi_version() >= 15 and all(hasattr(lib, n) for n in GYM_SYMBOLS)


def human_check(lib, line, seed=0):
    """None if the gym record `line` builds into its fight on `seed`, else the engine's reason."""
    if lib.sts2_human_check(line.encode(), int(seed)) == STS2_OK:
        return None
    return last_error(lib)


def has_obs_v2(lib):
    """Whether `lib` writes the obs v2 section this binding reads (ABI 10 on: RLN22's plan
    column after RLN21's facing and shown numbers; an ABI 8-9 library's section is shorter)."""
    return lib.sts2_abi_version() >= 10


def has_obs_full(lib):
    """Whether `lib` writes the full-view section (ABI 12 on, RLN30)."""
    return lib.sts2_abi_version() >= 12


def has_discard_potion(lib):
    """Whether `lib` has the discard-potion actions and `plan_depth` (ABI 13 on, RLN32)."""
    return lib.sts2_abi_version() >= 13


def has_obs_events(lib):
    """Whether `lib` writes the event section (ABI 14 on, RLN33)."""
    return lib.sts2_abi_version() >= 14


def has_content_stats(lib):
    """Whether `lib` has the content stats (ABI 11 on)."""
    return lib.sts2_abi_version() >= 11 and all(hasattr(lib, n) for n in CONTENT_STATS_SYMBOLS)


def content_stats(lib):
    """The content stats (`sts2_rl::content_stats`): `(columns, tables)`, `columns` a list of
    `(name, magnitude)` and `tables` a dict of int16 arrays (rows by vocab id x columns, copies):
    `cards`, `cards_up`, `relics`, `potions`."""
    if not has_content_stats(lib):
        raise StaleBuild(f"content stats need ABI 11, the library has {lib.sts2_abi_version()}")
    cols = [(c.rsplit(":", 1)[0], c.endswith(":m"))
            for c in lib.sts2_content_stats_columns().decode().split(",")]
    tables = {}
    for i, name in enumerate(("cards", "cards_up", "relics", "potions")):
        rows = ctypes.c_size_t()
        ptr = lib.sts2_content_stats(i, ctypes.byref(rows))
        if not ptr:
            raise Sts2Error(STS2_ERR_ARGUMENT, last_error(lib))
        arr = (ctypes.c_int16 * (rows.value * len(cols))).from_address(ptr)
        tables[name] = np.frombuffer(arr, dtype=np.int16).reshape(rows.value, len(cols)).copy()
    return cols, tables


def last_error(lib):
    return lib.sts2_last_error().decode(errors="replace")


def layout(lib):
    out = Layout()
    code = lib.sts2_layout(ctypes.byref(out))
    if code != STS2_OK:
        raise Sts2Error(code, last_error(lib))
    return out


def run_layout(lib):
    out = RunLayout()
    code = lib.sts2_run_layout(ctypes.byref(out))
    if code != STS2_OK:
        raise Sts2Error(code, last_error(lib))
    return out
