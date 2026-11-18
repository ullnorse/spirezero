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

RUN_ENDS = (
    "none", "died", "victory", "unsupported", "stuck", "capped", "fight_capped",
    "decision_capped", "practice_end", "gym_end",
)
"""What `Sts2RunEnvs.end` codes mean: 5 is the engine-action cap, 6 and 7 the end-to-end fight
and decision caps (losses, not deaths), 8 a practice episode's truncation at its first decision
outside combat (ABI 7), 9 a gym episode's end at its first decision outside combat (terminal,
ABI 15, RLN43)."""

PRACTICE_END = 8
GYM_END = 9

PHASE_REST, REST_AT, REST_SLOTS, REST_BASE, REST_HEAL = 7, 1648, 8, 236, 0
"""RLN47: the rest phase, the run obs's rest options ((RestOptionKind + 1, enabled) x 8), the
first rest action index and `RestOptionKind::Heal` (`run_obs.rs`, `run_action.rs`)."""

E2E = {"dedupe": True, "turn_cap": 100, "decision_cap": 6000, "events": True, "counters": True}
"""The end-to-end options of the Necrobinder plan (`sts2_rl::RunOptions::E2E`): pass
`**E2E` to `Sts2RunEnvs`."""

HOLD = 0xFFFF
"""An action that leaves its run env as it is for one step (`sts2_rl::HOLD`): with
`Sts2RunEnvs.restart`, a learner replays each env's run to where a checkpoint left it."""

EVENT_FIELDS = ("floors", "bosses", "boss_acts", "won")
"""The columns of `Sts2RunEnvs.events`."""

ENTITY_KINDS = (
    "hand_card", "combat_potion", "end_turn", "choice_option", "choice_done", "ancient_option",
    "event_option", "map_column", "reward", "reward_card", "reward_alternative", "reward_skip",
    "rest_option", "belt_potion", "select_option", "select_confirm", "select_cancel",
    "treasure_relic", "shop_card", "shop_relic", "shop_potion", "shop_remove", "bundle",
    "proceed", "discard_potion",
)
"""The entity kinds of `action_table()`'s first column (`sts2_rl::entity::EntityKind`)."""


def action_table(lib=None):
    """The static action -> entity table (`sts2_rl::entity`): an (actions, 4) uint8 array of
    entity kind (`ENTITY_KINDS`), slot, sub-slot (a target slot or a reward's card) and phase
    (`run_obs::phase_code`, 255 for any), per index of the run space (a copy)."""
    lib = lib or ffi.load()
    actions = int(ffi.run_layout(lib).actions)
    return _view(lib.sts2_run_action_table(), (actions, 4), np.uint8, False).copy()


class Sts2RunEnvs:
    """`n` run envs stepped together (`sts2_rl::RunEnvs`): whole runs as episodes,
    auto-resetting finished ones.

    Seeds: `seeds` (a list, played once or cycled with `cycle`), `seed_range` (`(start, end)`),
    else endless random seeds from `master`. `character` is a name or select-order index (None:
    seed `s` plays the `s % 5`th), `combat` is "agent", "greedy" or "random". A step is one agent
    decision per env; decisions with one legal option are taken inside the env. The arrays
    `obs` (n × obs_bytes, uint8), `masks` (n × mask_words, uint64), `done`, `won`, `end` (n,
    uint8, `RUN_ENDS`) and `floor` (n, uint16) are views on the engine's buffers.

    End-to-end options (all off by default; `**E2E` turns on the plan's): `dedupe` merges the
    mask into action classes (one index per class; `class_counts`, n × actions uint8, has each
    kept index's member count) and takes a decision with one class inside the env; `turn_cap`
    and `decision_cap` end a run as `fight_capped` / `decision_capped` (0: no cap); `events`
    fills `events` (n × 4 uint8, `EVENT_FIELDS`: floors entered, bosses killed, a bitmask of the
    acts whose boss died, won) for the last step; `counters` adds saturation and unmapped counts
    to `stats()`; `priv` fills `priv` (n × `priv_bytes` uint8, `sts2_rl::priv_obs`: the true draw
    top, next moves, this act's rooms, the next act's boss and map) for the value head only, zero
    otherwise. `obs_v2` (ABI 8) writes the obs v2 section (`sts2_rl::obs_v2`, at `layout.v2_at`);
    `obs_full` (ABI 12, with `obs_v2`) the full-view section (`sts2_rl::obs_full`, at
    `layout.full_at`). `discard_potion` (ABI 13, RLN32) lists the discard actions (indices after
    `PROCEED`) on a rewards screen with a potion or a shop with an affordable one while the belt is
    full; `plan_depth` (1 or 2, ABI 13) is the plan column's plays per plan. `obs_events` (ABI 14,
    with `obs_v2`) writes the event section (`sts2_rl::obs_events`, at `layout.events_at`).
    """

    def __init__(self, n, seeds=None, seed_range=None, master=0, cycle=False,
                 character="Necrobinder", ascension=0, combat="agent", intents=True, threads=1,
                 lib=None, vocab_hash=None, writeable=False, dedupe=False, turn_cap=0,
                 decision_cap=0, events=False, counters=False, priv=False, obs_v2=False,
                 obs_full=False, discard_potion=False, plan_depth=1, obs_events=False,
                 no_full_heal=False):
        self._ptr = None
        # RLN47: Python-side mask rule (`_mask_full_heal`); the masks view must be writeable.
        self.no_full_heal = bool(no_full_heal)
        self.full_heal_masked = 0
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
        config.dedupe = int(bool(dedupe))
        config.turn_cap = turn_cap
        config.decision_cap = decision_cap
        config.events = int(bool(events))
        config.counters = int(bool(counters))
        config.priv_obs = int(bool(priv))
        if obs_v2 and not ffi.has_obs_v2(self.lib):
            raise ffi.StaleBuild(
                f"obs_v2 needs ABI 10, the library has {self.lib.sts2_abi_version()}")
        config.obs_v2 = int(bool(obs_v2))
        self.obs_v2 = bool(obs_v2)
        if obs_full and not (obs_v2 and ffi.has_obs_full(self.lib)):
            raise ffi.StaleBuild(
                f"obs_full needs obs_v2 and ABI 12, the library has {self.lib.sts2_abi_version()}")
        config.obs_full = int(bool(obs_full))
        if (discard_potion or plan_depth != 1) and not ffi.has_discard_potion(self.lib):
            raise ffi.StaleBuild(
                f"discard_potion and plan_depth need ABI 13, the library has "
                f"{self.lib.sts2_abi_version()}")
        if plan_depth not in (1, 2):
            raise ValueError(f"plan_depth is 1 or 2, not {plan_depth}")
        config.discard_potion = int(bool(discard_potion))
        config.plan_depth = plan_depth
        self.obs_full = bool(obs_full)
        if obs_events and not (obs_v2 and ffi.has_obs_events(self.lib)):
            raise ffi.StaleBuild(
                f"obs_events needs obs_v2 and ABI 14, the library has {self.lib.sts2_abi_version()}")
        config.obs_events = int(bool(obs_events))
        self.obs_events = bool(obs_events)
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
                           writeable or self.no_full_heal)
        self.done = _view(lib.sts2_run_envs_done_ptr(ptr), (n,), np.uint8, writeable)
        self.won = _view(lib.sts2_run_envs_won_ptr(ptr), (n,), np.uint8, writeable)
        self.end = _view(lib.sts2_run_envs_end_ptr(ptr), (n,), np.uint8, writeable)
        self.floor = _view(lib.sts2_run_envs_floor_ptr(ptr), (n,), np.uint16, writeable)
        self.class_counts = _view(lib.sts2_run_envs_class_counts_ptr(ptr), (n, self.actions),
                                  np.uint8, writeable)
        self.events = _view(lib.sts2_run_envs_events_ptr(ptr), (n, 4), np.uint8, writeable)
        self.priv_bytes = int(self.layout.priv_bytes)
        # For the value head only: zero unless `priv=True`; a policy must never read it.
        self.priv = _view(lib.sts2_run_envs_priv_ptr(ptr), (n, self.priv_bytes), np.uint8,
                          writeable)
        # Practice (ABI 7; None with an ABI-6 library): the ended episode's mask and priv (with
        # `final_obs`, the position a truncation bootstraps from), and per env the cell code
        # (0 a normal run, else `ffi.CELL_NAMES` index + 1) of its episode and of the one that
        # ended in the last step.
        self.has_practice = ffi.has_practice(lib)
        self.final_masks = self.final_priv = self.cell = self.ended_cell = None
        if self.has_practice:
            self.final_masks = _view(lib.sts2_run_envs_final_mask_ptr(ptr),
                                     (n, self.mask_words), np.uint64, writeable)
            self.final_priv = _view(lib.sts2_run_envs_final_priv_ptr(ptr),
                                    (n, self.priv_bytes), np.uint8, writeable)
            self.cell = _view(lib.sts2_run_envs_cell_ptr(ptr), (n,), np.uint8, writeable)
            self.ended_cell = _view(lib.sts2_run_envs_ended_cell_ptr(ptr), (n,), np.uint8,
                                    writeable)
        self._actions = np.zeros(n, dtype=np.uint16)
        self._actions_ptr = self._actions.ctypes.data_as(ctypes.POINTER(ctypes.c_uint16))
        self._mask_full_heal()

    def _mask_full_heal(self):
        """RLN47 (`no_full_heal`): at a rest site with the player at full HP, drop Heal from the
        mask when another enabled option (Smith...) is offered: healing at full HP does nothing.
        Edits the mask in place after every reset and step (the library rewrites it each time)."""
        if not self.no_full_heal:
            return
        o = self.obs
        rows = np.nonzero(o[:, 3] == PHASE_REST)[0]
        if not len(rows):
            return
        hp = o[rows, 8].astype(np.int32) | o[rows, 9].astype(np.int32) << 8
        mx = o[rows, 10].astype(np.int32) | o[rows, 11].astype(np.int32) << 8
        for i in rows[(mx > 0) & (hp >= mx)]:
            kind = o[i, REST_AT:REST_AT + 2 * REST_SLOTS:2]
            en = o[i, REST_AT + 1:REST_AT + 2 * REST_SLOTS:2] > 0
            heal = np.nonzero((kind == REST_HEAL + 1) & en)[0]
            if not len(heal) or not ((kind > 0) & (kind != REST_HEAL + 1) & en).any():
                continue
            for slot in heal:
                a = REST_BASE + int(slot)
                bit = np.uint64(1) << np.uint64(a % 64)
                if self.masks[i, a // 64] & bit:
                    self.masks[i, a // 64] &= ~bit
                    self.full_heal_masked += 1

    def _check(self, code):
        if code < 0:
            raise ffi.Sts2Error(code, ffi.last_error(self.lib))
        return code

    def step(self, actions):
        """Applies one action index per env; the number of runs that ended."""
        np.copyto(self._actions, actions, casting="unsafe")
        ended = self._check(self.lib.sts2_run_envs_step(self._ptr, self._actions_ptr, self.n))
        self._mask_full_heal()
        return ended

    def reset_all(self):
        self._check(self.lib.sts2_run_envs_reset_all(self._ptr))
        self._mask_full_heal()

    def set_practice(self, slots=0, weights=(0,) * 6, pool=32, max_uses=4, min_active=8,
                     capture=True, seed=0, off=False):
        """Practice starts (`sts2_rl::practice`): envs `0..slots` start a reseeded snapshot of
        a cell drawn by `weights` (integers, by `ffi.CELL_NAMES`) when their episode ends;
        `capture` files normal runs' snapshots. `off=True` stops both (the pool stays)."""
        if not self.has_practice:
            raise ffi.StaleBuild("practice needs an ABI-7 sts2-ffi build; rebuild the library")
        if off:
            self._check(self.lib.sts2_run_envs_set_practice(self._ptr, None))
            return
        c = ffi.PracticeConfig()
        c.slots = int(slots)
        for i, w in enumerate(weights):
            c.weights[i] = int(w)
        c.pool, c.max_uses, c.min_active = int(pool), int(max_uses), int(min_active)
        c.capture, c.seed = int(bool(capture)), int(seed)
        self._check(self.lib.sts2_run_envs_set_practice(self._ptr, ctypes.byref(c)))

    def start_record(self, i):
        """Env `i`'s fight start as a gym record line (`sts2_rl::human::start_record`), or None
        when it is not in a combat room (NAZ1)."""
        buf = ctypes.create_string_buffer(8192)
        n = self.lib.sts2_run_envs_start_record(self._ptr, i, buf, len(buf))
        if n < 0:
            self._check(int(n))
        if n == 0:
            return None
        if n >= len(buf):
            buf = ctypes.create_string_buffer(int(n) + 1)
            self.lib.sts2_run_envs_start_record(self._ptr, i, buf, len(buf))
        return buf.value.decode()

    def load_gym(self, path):
        """Loads the gym's starts (`sts2_rl::human`, JSON lines); the number loaded."""
        if not ffi.has_gym(self.lib):
            raise ffi.StaleBuild("the gym needs an ABI-15 sts2-ffi build; rebuild the library")
        return self._check(self.lib.sts2_run_envs_load_gym(self._ptr, str(path).encode()))

    def set_gym(self, first=0, slots=0, weights=(0,) * 9, variants=False, hall_hp_pct=0,
                sequential=False, seed=0, off=False):
        """Gym starts (RLN43): envs `first..first + slots` start a human fight of a cell drawn by
        `weights` (by `ffi.GYM_CELL_NAMES`) when their episode ends; `sequential` hands out the
        records in order, each once, on seeds `seed + k`. `off=True` gives it no slots."""
        if off:
            self._check(self.lib.sts2_run_envs_set_gym(self._ptr, None))
            return
        c = ffi.GymConfig()
        c.first, c.slots = int(first), int(slots)
        for i, w in enumerate(weights):
            c.weights[i] = int(w)
        c.variants, c.hall_hp_pct = int(bool(variants)), int(hall_hp_pct)
        c.sequential, c.seed = int(bool(sequential)), int(seed)
        self._check(self.lib.sts2_run_envs_set_gym(self._ptr, ctypes.byref(c)))

    def gym_stats(self):
        """Per gym cell counts: a dict of name -> list of 9 (`ffi.GYM_CELL_NAMES` order)."""
        out = ffi.GymStats()
        self._check(self.lib.sts2_run_envs_gym_stats(self._ptr, ctypes.byref(out)))
        return {name: list(getattr(out, name)) for name, _ in ffi.GymStats._fields_}

    def practice_stats(self):
        """Per-cell practice counts: a dict of name -> list of 6 (`ffi.CELL_NAMES` order)."""
        out = ffi.PracticeStats()
        self._check(self.lib.sts2_run_envs_practice_stats(self._ptr, ctypes.byref(out)))
        return {name: list(getattr(out, name)) for name, _ in ffi.PracticeStats._fields_}

    def copy(self, src, dst, reseed=None):
        """Env `dst[i]` becomes a copy of env `src[i]`'s position (run, streams, counters,
        obs, mask), pair by pair in order: a mid-fight state copy (RLN27).

        `reseed` (one seed per pair, or None): a copy in a fight then hides what the player
        can't know: the draw pile is reshuffled and the fight's random streams reseeded from
        `reseed[i]` (`Combat::determinize`), so it plays a plausible future, not the real one
        (RLN28). Everything in the observation stays as it is."""
        src = np.ascontiguousarray(src, dtype=np.uint32)
        dst = np.ascontiguousarray(dst, dtype=np.uint32)
        p = ctypes.POINTER(ctypes.c_uint32)
        seeds = None
        if reseed is not None:
            seeds = np.ascontiguousarray(reseed, dtype=np.uint64)
            if seeds.shape != src.shape:
                raise ValueError("reseed needs one seed per copy")
            seeds = seeds.ctypes.data_as(ctypes.POINTER(ctypes.c_uint64))
        self._check(self.lib.sts2_run_envs_copy(
            self._ptr, src.ctypes.data_as(p), dst.ctypes.data_as(p), seeds, len(src)))

    def restart(self, seeds, next_serial):
        """Env `i` starts a fresh run of `seeds[i]`; the runs that end after this take the
        source's episodes from serial `next_serial` on (a resume: replay each env's actions,
        `HOLD` for the envs whose history is shorter)."""
        seeds = np.ascontiguousarray(seeds, dtype=np.uint64)
        self._check(self.lib.sts2_run_envs_restart(
            self._ptr, seeds.ctypes.data_as(ctypes.POINTER(ctypes.c_uint64)), len(seeds),
            int(next_serial)))

    def stats(self):
        """What the last step did: actions, decisions, auto, resolved, refused, unmapped,
        ended, won, capped, stuck, reset, fight_capped, decision_capped, merged,
        unmapped_classes, auto_class, unmapped_decisions, and `saturated` as a dict over
        `ffi.SATURATIONS`."""
        out = ffi.RunStepStats()
        self._check(self.lib.sts2_run_envs_stats(self._ptr, ctypes.byref(out)))
        stats = {name: getattr(out, name) for name, _ in ffi.RunStepStats._fields_}
        stats["saturated"] = dict(zip(ffi.SATURATIONS, list(out.saturated)))
        return stats

    def legal(self):
        """The masks as bools, n × actions (a copy)."""
        bits = np.unpackbits(self.masks.view(np.uint8), axis=1, bitorder="little")
        return bits[:, : self.actions].astype(bool)

    def close(self):
        if self._ptr:
            self.lib.sts2_run_envs_free(self._ptr)
            self._ptr = None
            self.obs = self.final_obs = self.masks = self.done = self.won = self.end = None
            self.floor = self.class_counts = self.events = None
            self.final_masks = self.final_priv = self.cell = self.ended_cell = None

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
