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

ROOT = pathlib.Path(__file__).resolve().parents[2]
"""The workspace root."""

ABI_VERSION = 3

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
    ]


class RunLayout(ctypes.Structure):
    _fields_ = [
        (name, ctypes.c_uint64)
        for name in (
            "obs_bytes",
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
        "sts2_run_envs_stats": ([envs, c.POINTER(RunStepStats)], c.c_int32),
        "sts2_run_envs_len": ([envs], c.c_size_t),
        "sts2_run_envs_obs_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_final_obs_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_mask_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_done_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_won_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_end_ptr": ([envs], c.c_void_p),
        "sts2_run_envs_floor_ptr": ([envs], c.c_void_p),
    }
    for name, (args, result) in signatures.items():
        f = getattr(lib, name)
        f.argtypes = args
        f.restype = result
    return lib


def check_build(lib, expected_build_id=None):
    """Raises `StaleBuild` unless `lib` has this module's ABI and was built from
    `expected_build_id`'s sources (default: this checkout's)."""
    abi = lib.sts2_abi_version()
    if abi != ABI_VERSION:
        raise StaleBuild(f"the library has ABI {abi}, this binding {ABI_VERSION}")
    built = lib.sts2_build_id().decode()
    expected = expected_build_id or source_hash()
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
