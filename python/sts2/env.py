"""Vectorised Slay the Spire 2 environments over the C ABI.

Combat: `Sts2Envs` is the batch with zero-copy numpy views on the engine-owned buffers;
`Sts2PufferEnv` is a PufferLib native environment (`pufferlib.PufferEnv`) over it. Observations
are the raw little-endian bytes of `docs/specs/rl-2x-plan.md`, section 7.1 (featurise them in
numpy or torch on the batch); actions are indices of the 140-entry space of section 7.2, and a
policy must pick from the legal mask.

Whole runs (stage 9): `Sts2RunEnvs` and `Sts2RunPufferEnv`, the same over `sts2_rl::RunEnvs`:
the run space appends the run-level choices after the combat indices, and the observation is
`crates/sts2-rl/src/run_obs.rs`'s.
"""

import ctypes

import numpy as np

from . import ffi


def _view(address, shape, dtype, writeable):
    """A numpy array over `address` (zero copy; it does not own the memory)."""
    dtype = np.dtype(dtype)
    count = int(np.prod(shape))
    buffer = (ctypes.c_char * (count * dtype.itemsize)).from_address(address)
    array = np.frombuffer(buffer, dtype=dtype, count=count).reshape(shape)
    array.flags.writeable = writeable
    return array


class Sts2Envs:
    """`n` combat envs stepped together (`sts2_rl::Envs`), auto-resetting finished episodes.

    `starts` is a JSON-lines start corpus path (`cargo xtask sweep --emit-starts`) or None for
    the RL bench's W1-all starts; `seed` draws the episode order; `threads` steps the batch on
    that many threads inside one call. The arrays `obs` (n × obs_bytes, uint8), `static_obs`,
    `masks` (n × mask_words, uint64), `done` and `won` (n, uint8) and `final_hp` (n, int16: the
    player's HP when an episode ended in the last step) are views on the engine's
    buffers: they change in place on every `step`/`reset_all` and must not be used after
    `close`. `vocab_hash`, if given, must be the library's (`sts2_vocab_hash()`): a net trained
    on one vocabulary refuses another.
    """

    def __init__(self, n, starts=None, seed=0, threads=1, lib=None, vocab_hash=None,
                 writeable=False):
        self._ptr = None
        self.lib = lib or ffi.load()
        if vocab_hash is not None and self.lib.sts2_vocab_hash() != vocab_hash:
            raise ffi.StaleBuild(
                f"vocabulary {self.lib.sts2_vocab_hash():016x}, expected {vocab_hash:016x}"
            )
        self.layout = ffi.layout(self.lib)
        path = None if starts is None else str(starts).encode()
        self._ptr = self.lib.sts2_envs_new(n, path, seed, threads)
        if not self._ptr:
            raise ffi.Sts2Error(ffi.STS2_ERR_ARGUMENT, ffi.last_error(self.lib))
        self.n = n
        self.obs_bytes = int(self.layout.obs_bytes)
        self.actions = int(self.layout.actions)
        self.mask_words = int(self.layout.mask_words)
        lib, ptr = self.lib, self._ptr
        static_bytes = int(self.layout.static_bytes)
        self.obs = _view(lib.sts2_envs_obs_ptr(ptr), (n, self.obs_bytes), np.uint8, writeable)
        self.static_obs = _view(lib.sts2_envs_static_ptr(ptr), (n, static_bytes), np.uint8,
                                writeable)
        self.masks = _view(lib.sts2_envs_mask_ptr(ptr), (n, self.mask_words), np.uint64,
                           writeable)
        self.done = _view(lib.sts2_envs_done_ptr(ptr), (n,), np.uint8, writeable)
        self.won = _view(lib.sts2_envs_won_ptr(ptr), (n,), np.uint8, writeable)
        self.final_hp = _view(lib.sts2_envs_final_hp_ptr(ptr), (n,), np.int16, writeable)
        self._actions = np.zeros(n, dtype=np.uint16)
        self._actions_ptr = self._actions.ctypes.data_as(ctypes.POINTER(ctypes.c_uint16))

    def _check(self, code):
        if code < 0:
            raise ffi.Sts2Error(code, ffi.last_error(self.lib))
        return code

    def blindfold(self, seed):
        """From the next reset on, episodes start with the hidden part (draw order, RNG
        streams) resampled from `seed` and the episode number; `None` turns it off."""
        self._check(self.lib.sts2_envs_set_blindfold(self._ptr, int(seed is not None),
                                                     0 if seed is None else int(seed)))

    def step(self, actions):
        """Applies one action index per env; the number of episodes that ended."""
        np.copyto(self._actions, actions, casting="unsafe")
        return self._check(self.lib.sts2_envs_step(self._ptr, self._actions_ptr, self.n))

    def reset_all(self):
        self._check(self.lib.sts2_envs_reset_all(self._ptr))

    def stats(self):
        """What the last step did: actions, unmapped, ended, won, capped, reset."""
        out = ffi.StepStats()
        self._check(self.lib.sts2_envs_stats(self._ptr, ctypes.byref(out)))
        return {name: getattr(out, name) for name, _ in ffi.StepStats._fields_}

    def legal(self):
        """The masks as bools, n × actions (a copy)."""
        bits = np.unpackbits(self.masks.view(np.uint8), axis=1, bitorder="little")
        return bits[:, : self.actions].astype(bool)

    def close(self):
        if self._ptr:
            self.lib.sts2_envs_free(self._ptr)
            self._ptr = None
            self.obs = self.static_obs = self.masks = self.done = self.won = None
            self.final_hp = None

    def __del__(self):
        self.close()


def _spaces():
    """`(Box, Discrete)`: gymnasium's (what PufferLib checks against), else minimal ones."""
    try:
        import gymnasium

        return gymnasium.spaces.Box, gymnasium.spaces.Discrete
    except ImportError:

        class Box:
            def __init__(self, low, high, shape, dtype):
                self.low, self.high, self.shape, self.dtype = low, high, shape, np.dtype(dtype)

        class Discrete:
            def __init__(self, n):
                self.n, self.shape, self.dtype = n, (), np.dtype(np.int64)

        return Box, Discrete


try:
    from pufferlib import PufferEnv as _PufferBase
except ImportError:

    class _PufferBase:
        """The part of `pufferlib.PufferEnv` this env relies on, for use without PufferLib."""

        def __init__(self, buf=None):
            n = self.num_agents
            if buf is None:
                buf = {
                    "observations": np.zeros((n, *self.single_observation_space.shape),
                                             dtype=self.single_observation_space.dtype),
                    "rewards": np.zeros(n, dtype=np.float32),
                    "terminals": np.zeros(n, dtype=bool),
                    "truncations": np.zeros(n, dtype=bool),
                    "masks": np.ones(n, dtype=bool),
                    "actions": np.zeros(n, dtype=np.int32),
                }
            for name, array in buf.items():
                setattr(self, name, array)
            self.agent_ids = np.arange(n)


class Sts2PufferEnv(_PufferBase):
    """A PufferLib native env: `num_envs` combats, one agent each.

    Without `buf` the observations are the engine's buffer itself (zero copy); with PufferLib's
    shared `buf` (its multiprocessing backend) they are copied into it after each step. The
    reward is +1 for a won episode, -1 for any other end (a loss, unported content, the action
    cap) and 0 otherwise; `done`, `won`, HP and block are all in the observation for other
    shaping (`batch.final_hp` is the HP an episode ended on). With `with_static` each
    observation is the dynamic part followed by the static one (copied in). `action_masks` (num_envs × 140 bools) is the legal mask after the last step; an
    illegal action raises `Sts2Error` and nothing is applied.
    """

    def __init__(self, num_envs=64, starts=None, seed=0, threads=1, buf=None, lib=None,
                 vocab_hash=None, with_static=False):
        self.batch = Sts2Envs(num_envs, starts, seed, threads, lib, vocab_hash, writeable=True)
        Box, Discrete = _spaces()
        self.with_static = with_static
        width = self.batch.obs_bytes + (self.batch.static_obs.shape[1] if with_static else 0)
        self.single_observation_space = Box(low=0, high=255, shape=(width,), dtype=np.uint8)
        self.single_action_space = Discrete(self.batch.actions)
        self.num_agents = num_envs
        self.zero_copy = buf is None and not with_static
        if buf is None and with_static:
            buf = {
                "observations": np.zeros((num_envs, width), dtype=np.uint8),
                "rewards": np.zeros(num_envs, dtype=np.float32),
                "terminals": np.zeros(num_envs, dtype=bool),
                "truncations": np.zeros(num_envs, dtype=bool),
                "masks": np.ones(num_envs, dtype=bool),
                "actions": np.zeros(num_envs, dtype=np.int32),
            }
        if buf is None:
            buf = {
                "observations": self.batch.obs,
                "rewards": np.zeros(num_envs, dtype=np.float32),
                "terminals": np.zeros(num_envs, dtype=bool),
                "truncations": np.zeros(num_envs, dtype=bool),
                "masks": np.ones(num_envs, dtype=bool),
                "actions": np.zeros(num_envs, dtype=np.int32),
            }
        super().__init__(buf)
        self.action_masks = self.batch.legal()

    def _publish(self):
        if self.with_static:
            width = self.batch.obs_bytes
            np.copyto(self.observations[:, :width], self.batch.obs)
            np.copyto(self.observations[:, width:], self.batch.static_obs)
        elif not self.zero_copy:
            np.copyto(self.observations, self.batch.obs)
        self.action_masks = self.batch.legal()

    def reset(self, seed=None):
        """Starts a new episode in every env. The episode order was drawn from the constructor's
        seed; `seed` is accepted for the PufferLib interface."""
        self.batch.reset_all()
        self.rewards[:] = 0
        self.terminals[:] = False
        self.truncations[:] = False
        self._publish()
        return self.observations, []

    def step(self, actions):
        ended = self.batch.step(actions)
        done = self.batch.done.astype(bool)
        np.copyto(self.terminals, done)
        self.rewards[:] = np.where(done, np.where(self.batch.won == 1, 1.0, -1.0), 0.0)
        self._publish()
        infos = []
        if ended:
            infos.append({"episodes": ended, "wins": int(self.batch.won.sum())})
        return self.observations, self.rewards, self.terminals, self.truncations, infos

    def close(self):
        if self.zero_copy:
            self.observations = None
        self.batch.close()


COMBAT_MODES = {"agent": 0, "greedy": 1, "random": 2}
"""Who plays the fights in a run env: the agent through the combat indices, or a resolver."""

CHARACTERS = ("Ironclad", "Silent", "Defect", "Regent", "Necrobinder")
"""Select order: a run env's `character` is a name, an index, or None for seed % 5."""

RUN_ENDS = ("none", "died", "victory", "unsupported", "stuck", "capped")
"""What `Sts2RunEnvs.end` codes mean."""


class Sts2RunEnvs:
    """`n` run envs stepped together (`sts2_rl::RunEnvs`): whole runs as episodes,
    auto-resetting finished ones.

    Seeds: `seeds` (a list, played once or cycled with `cycle`), `seed_range` (`(start, end)`),
    else endless random seeds from `master`. `character` is a name or select-order index (None:
    seed `s` plays the `s % 5`th), `combat` is "agent", "greedy" or "random". A step is one agent
    decision per env; decisions with one legal option are taken inside the env. The arrays
    `obs` (n × obs_bytes, uint8), `masks` (n × mask_words, uint64), `done`, `won`, `end` (n,
    uint8, `RUN_ENDS`) and `floor` (n, uint16) are views on the engine's buffers.
    """

    def __init__(self, n, seeds=None, seed_range=None, master=0, cycle=False,
                 character="Necrobinder", ascension=0, combat="agent", intents=True, threads=1,
                 lib=None, vocab_hash=None, writeable=False):
        self._ptr = None
        self.lib = lib or ffi.load()
        if vocab_hash is not None and self.lib.sts2_vocab_hash() != vocab_hash:
            raise ffi.StaleBuild(
                f"vocabulary {self.lib.sts2_vocab_hash():016x}, expected {vocab_hash:016x}"
            )
        self.layout = ffi.run_layout(self.lib)
        config = ffi.RunConfig()
        self._seeds = None
        if seeds is not None:
            self._seeds = np.ascontiguousarray(seeds, dtype=np.uint64)
            config.seed_kind = 0
            config.seeds = self._seeds.ctypes.data_as(ctypes.POINTER(ctypes.c_uint64))
            config.seeds_len = len(self._seeds)
        elif seed_range is not None:
            config.seed_kind = 1
            config.start, config.end = seed_range
        else:
            config.seed_kind = 2
            config.master = master
        config.cycle = int(bool(cycle))
        if character is None:
            config.character = -1
        elif isinstance(character, str):
            config.character = CHARACTERS.index(character)
        else:
            config.character = int(character)
        config.ascension = ascension
        config.combat = COMBAT_MODES[combat] if isinstance(combat, str) else int(combat)
        config.intents = int(bool(intents))
        self._ptr = self.lib.sts2_run_envs_new(n, ctypes.byref(config), threads)
        if not self._ptr:
            raise ffi.Sts2Error(ffi.STS2_ERR_ARGUMENT, ffi.last_error(self.lib))
        self.n = n
        self.obs_bytes = int(self.layout.obs_bytes)
        self.actions = int(self.layout.actions)
        self.mask_words = int(self.layout.mask_words)
        lib, ptr = self.lib, self._ptr
        self.obs = _view(lib.sts2_run_envs_obs_ptr(ptr), (n, self.obs_bytes), np.uint8,
                         writeable)
        self.final_obs = _view(lib.sts2_run_envs_final_obs_ptr(ptr), (n, self.obs_bytes),
                               np.uint8, writeable)
        self.masks = _view(lib.sts2_run_envs_mask_ptr(ptr), (n, self.mask_words), np.uint64,
                           writeable)
        self.done = _view(lib.sts2_run_envs_done_ptr(ptr), (n,), np.uint8, writeable)
        self.won = _view(lib.sts2_run_envs_won_ptr(ptr), (n,), np.uint8, writeable)
        self.end = _view(lib.sts2_run_envs_end_ptr(ptr), (n,), np.uint8, writeable)
        self.floor = _view(lib.sts2_run_envs_floor_ptr(ptr), (n,), np.uint16, writeable)
        self._actions = np.zeros(n, dtype=np.uint16)
        self._actions_ptr = self._actions.ctypes.data_as(ctypes.POINTER(ctypes.c_uint16))

    def _check(self, code):
        if code < 0:
            raise ffi.Sts2Error(code, ffi.last_error(self.lib))
        return code

    def step(self, actions):
        """Applies one action index per env; the number of runs that ended."""
        np.copyto(self._actions, actions, casting="unsafe")
        return self._check(self.lib.sts2_run_envs_step(self._ptr, self._actions_ptr, self.n))

    def reset_all(self):
        self._check(self.lib.sts2_run_envs_reset_all(self._ptr))

    def stats(self):
        """What the last step did: actions, decisions, auto, resolved, refused, unmapped,
        ended, won, capped, stuck, reset."""
        out = ffi.RunStepStats()
        self._check(self.lib.sts2_run_envs_stats(self._ptr, ctypes.byref(out)))
        return {name: getattr(out, name) for name, _ in ffi.RunStepStats._fields_}

    def legal(self):
        """The masks as bools, n × actions (a copy)."""
        bits = np.unpackbits(self.masks.view(np.uint8), axis=1, bitorder="little")
        return bits[:, : self.actions].astype(bool)

    def close(self):
        if self._ptr:
            self.lib.sts2_run_envs_free(self._ptr)
            self._ptr = None
            self.obs = self.final_obs = self.masks = self.done = self.won = self.end = self.floor = None

    def __del__(self):
        self.close()


class Sts2RunPufferEnv(_PufferBase):
    """A PufferLib native env: `num_envs` whole runs, one agent each.

    As `Sts2PufferEnv`, over `Sts2RunEnvs` (its keyword arguments pass through). The reward is
    +1 for a victory, -1 for any other end and 0 otherwise; the floor, HP and gold are in the
    observation for other shaping (and `batch.floor` holds the final floor of runs that just
    ended).
    """

    def __init__(self, num_envs=64, buf=None, **kwargs):
        self.batch = Sts2RunEnvs(num_envs, writeable=True, **kwargs)
        Box, Discrete = _spaces()
        self.single_observation_space = Box(
            low=0, high=255, shape=(self.batch.obs_bytes,), dtype=np.uint8
        )
        self.single_action_space = Discrete(self.batch.actions)
        self.num_agents = num_envs
        self.zero_copy = buf is None
        if buf is None:
            buf = {
                "observations": self.batch.obs,
                "rewards": np.zeros(num_envs, dtype=np.float32),
                "terminals": np.zeros(num_envs, dtype=bool),
                "truncations": np.zeros(num_envs, dtype=bool),
                "masks": np.ones(num_envs, dtype=bool),
                "actions": np.zeros(num_envs, dtype=np.int32),
            }
        super().__init__(buf)
        self.action_masks = self.batch.legal()

    def _publish(self):
        if not self.zero_copy:
            np.copyto(self.observations, self.batch.obs)
        self.action_masks = self.batch.legal()

    def reset(self, seed=None):
        """Starts a new run in every env (the next seeds of the source)."""
        self.batch.reset_all()
        self.rewards[:] = 0
        self.terminals[:] = False
        self.truncations[:] = False
        self._publish()
        return self.observations, []

    def step(self, actions):
        ended = self.batch.step(actions)
        done = self.batch.done.astype(bool)
        np.copyto(self.terminals, done)
        self.rewards[:] = np.where(done, np.where(self.batch.won == 1, 1.0, -1.0), 0.0)
        self._publish()
        infos = []
        if ended:
            infos.append({
                "runs": ended,
                "wins": int(self.batch.won.sum()),
                "floors": self.batch.floor[done].tolist(),
            })
        return self.observations, self.rewards, self.terminals, self.truncations, infos

    def close(self):
        if self.zero_copy:
            self.observations = None
        self.batch.close()
