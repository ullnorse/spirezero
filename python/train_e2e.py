"""The end-to-end Necrobinder PPO learner (`docs/plans/necro-e2e.md` §1, WP4).

One process, synchronous: collect `steps` decisions from each of `envs` run envs
(`Sts2RunEnvs(**E2E, priv=True)`, A10 Necrobinder, training seeds 100M-199M), compute chained
GAE over whole runs, then PPO on the batch, and repeat.

    cargo build --profile ffi -p sts2-ffi
    ~/spirebot/.venv/bin/python python/train_e2e.py --out ~/spirebot/target/rl-checkpoints/RLN5/x \\
        [--minutes 15] [--resume]

**Reward** (`Reward`, from the env's WP1 events; γ = 1): +1.0 for a won run, +0.05 per boss
killed, +0.002 per floor entered; a death or a cap is 0, with no HP term (spire-mentor's
pre-registered Instinct reward). The reward of a decision is what its step's events report.

**Returns:** GAE with γ = 1 over each env's rows in order, chained across fights and the run
(no cut at a fight's end); λ is per row, 0.95 on combat rows and 0.97 on run rows; a run's end
is terminal (value 0, caps included); the batch's last rows bootstrap from the value of the
next state. Advantages are normalised per row type (combat, run) over the batch.
`returns="two_chain"` (RLN15, `sts2.e2e.returns`, spire-mentor's design) instead runs a combat
chain (λ 0.95, within a fight, bootstrapping into the next run row) and a run chain over the run
rows alone (λ 0.97, rewards summed in between); run rows of open runs are held back until their
run ends or for `hold_iters` iterations, and a resume drops them. `adv_norm="per_type"` uses
their per-decision-type moments (decay 5e-4 per row; a type under 64 rows uses its family's
batch pool), saved in `state.pt`. Both are settable with `--set` at a resume.

**PPO:** clip 0.2, dual clip 3, value loss 0.5·MSE (no clip), entropy per row type over the
mask's classes / log(classes), one AdamW (trunk and policy lr 1e-4, value module 3e-4,
weight decay 1e-4, grad clip 1.0, 10 warm-up iterations), 131,072 decisions per iteration,
minibatch 8,192, E=2 epochs until `e1_at` decisions (250M), then E=1. The entropy coefficient
is flat at its high end for 100M decisions, then geometric to its low end at 4B (combat 0.01 ->
0.002, run 0.0125 -> 0.006).

**Upgrade bonus** (RLN40, `reward.upgrade`, 0 = off and bit-identical): + `upgrade` per upgrade
level the deck gains in a step (the deck section of the run obs before and after; a run's last
step counts its final deck). Smith's gain (+1.74 floors at HP >= 70%, RLN39 §4) arrives
fights later and is far below the return's noise, so the run head sees no signal for it
(`docs/research/rln40-trial-smith.md`); the bonus puts that credit on the decision itself.
`--set reward='{"upgrade": 0.001}'` at a resume.

**Entropy collapse rule** (RLN35, `ent_boost`, off by default; off is bit-identical;
spire-mentor's P5): per row type, if the update's mean normalised entropy (`ent_combat`,
`ent_run`) stays under `ent_boost_floor` (0.10 combat, 0.05 run) for `ent_boost_iters` (20)
consecutive iterations, that type's coefficient is multiplied by 1.5 and its streak resets; at
most `ent_boost_max` (2) times per type. The multipliers, streaks and boost counts are saved in
`state.pt` (an older checkpoint starts at 1) and logged as `ent_mult`; the parameters are
settable at a resume.

**KL control:** a fixed probe of 8,192 rows (the first iteration's first rows) is scored
before each update and after each epoch: KL above 0.06 stops the iteration's remaining epochs,
and the lr factor m goes ×1.5 under 0.005 and ÷1.5 over 0.02, within [0.3, 2].

RLN14: each iteration also scores a probe redrawn from its own batch (24 token-width strata,
8,192 rows, as spire-mentor's) and logs it as `kl_batch`. `kl_source` ("fixed", the default, or
"batch"; settable at a resume) picks which of the two drives the early stop and the lr factor.

**EMA:** two copies of the weights, 0.996 and 0.999 per iteration. bf16 autocast; fp32
weights, optimiser, log-softmax and value outputs.

**Checkpoints** (`<out>/state.pt`, every `ckpt_every` decisions and at the end): the net, the
optimiser, both EMAs, every RNG, the decision and iteration counters, the lr factor, the probe,
the finished runs' floors, and the envs' position: each env's current seed, the source's next
serial and the actions of each env's current run. A resume restarts each env on its seed and
replays its actions (`sts2.HOLD` for envs with shorter histories), so a killed and resumed
learner goes on exactly as it would have.

**Speed flags** (RLN9; `Config`, recorded in `state.pt`): `sort_micro` splits each minibatch
(the same rows as before) into micros of similar token width, so padding drops; `cache_tokens`
keeps the collect's tokens packed in narrow dtypes (`encode.pack`) instead of re-encoding every
micro; `value_first` runs the value block for its read position only; `fused_adam`; and
`probe_chunk` scores the KL probe in pieces; `compile` (RLN10, off unless it measured faster)
compiles the transformer blocks with torch.compile (inductor, dynamic shapes; the collect forward
shares them). Triton compiles a C launcher against `Python.h`, which this host's Python lacks
and there is no sudo, so the headers of Ubuntu's `libpython3.14-dev` (same 3.14.4 build,
unpacked with `apt download` + `dpkg -x`, no root) live in `~/.local/pyinclude`; importing this
module appends them to `CPATH` (and `tools/e2e_lane.sh` exports the same). If the headers or the
compiled build are missing or fail on a dry forward/backward, the learner logs it and goes on
eager. Each is the same maths (gradients agree to float
noise); with all of them off (and `probe_chunk=0`) the weights are bit-identical to RLN5's.
`overlap` (off by default) collects batch N+1 on a second thread and CUDA stream with the
weights from before update N, which batch N+1 then trains on (one iteration of policy lag; the
ratio uses the stored behaviour logprobs); its checkpoint holds the behaviour weights and the
position from before the batch collected ahead, which a resume collects again. Expandable
CUDA segments are on, since variable widths fragment the allocator.

**RLN44 speed flags** (`Config`, off by default and bit-identical off; all settable at a
resume; `docs/research/rln44-fast-ppo.md`): `fast_gather` (content-stats and event gathers by
`F.embedding`: an advanced-index backward serialised duplicate indices, 46% of a minibatch);
`fixed_probe=false` (needs `kl_source="batch"`; skips the fixed probe, `kl` logs NaN);
`static_head` (the head's legal pairs by `nonzero_static` with host-counted sizes, no syncs);
`pinned_copies` (non_blocking host->device copies in the collect); `compile_encode` (the
collect's encode compiled at its fixed batch size); `compile_net` (needs `compile` and
`static_head`: forward and policy compiled whole). `epochs` and `e1_at` are settable at a
resume. Gradients agree with the base to the noise the block compile already has.
`compile_net` runs without duck shaping (RLN45: equal sizes in a gym batch shared one symbol and
inductor failed on a view of their product); a graph that still fails to build runs that call
eager, logged once (`_guarded`).

**HP potential in practice** (RLN44, `practice.hp_coef`, 0 = off and bit-identical): a practice
row gets `hp_coef`·(φ after − φ before), φ = HP / max HP (the live combat HP in a fight, 0 at
a death), so an episode's sum is −`hp_coef`·HP lost / max HP. Normal runs are unchanged.

**Practice** (RLN12, `Practice`, off by default; spire-mentor's Instinct design, plan row
"Practice"): normal runs capture the run just before each map move into an elite or boss point
(`sts2_rl::practice`, a pool of the 32 most recent per cell); envs `0..slots` start a snapshot,
reseeded (new run and player streams: same encounter, new shuffles, AI and rewards), when their
episode ends. Over `span` decisions from `start` (the decision count when practice was turned
on), the slots' share goes 40% -> 25% and the bosses' share of starts 40% -> 75% (bosses 15/20/65
by act, elites 20/40/40; an empty cell falls back to lower acts, then to a normal run). A practice
episode is ordinary rows (honest obs; priv value-only) with the same reward; it ends at its first
decision outside combat, a truncation whose row gets the value of that position as bootstrap
(the act-3 boss plays to the run's end: at A10 both of the act's bosses). Its floors and wins
stay out of the run statistics. `practice` in the log has per cell `ended` [episodes, fight
survived: truncated, won, or a boss cell's boss killed], `ends` (how they ended, `RUN_ENDS`
names) and for boss cells `kills` (episodes that killed 0, 1, 2 bosses); RLN34: before, an
act-3 boss start counted as survived only when it won the run (both A10 bosses). Off, the learner is the same maths to the bit, and it
also runs on an ABI-6 library. `--set practice='{"enabled": true}'` turns it on at a resume.
RLN37 (`adaptive`, off by default, arXiv 2611.12088): the cells' weights follow where normal runs
die (last obs's act and room: elite or boss; hallway deaths are only counted, `deaths.other`):
every `adaptive_every` (10) iterations L = `adaptive_decay` (0.5) * L + deaths per cell, weights
(1 - `adaptive_floor`) L / sum L + floor / 6 (0.1), replacing `elite_mix`, `boss_mix` and
`boss_share` (the fixed mix until the first update); `share` is unchanged. L and the window's
deaths are in state.pt (an older checkpoint starts at 0), `practice.adaptive` in the log has L,
the weights and the collect's deaths. `--set practice='{"adaptive": true}'` at a resume.

**Combat gym** (RLN43, `practice.human`, `Human`; off by default and bit-identical): a share of
the practice slots play fights from strong players' A10 runs (`rln43_gym.py build`): their deck,
relics, potions, HP, act, floor and encounter, on a fresh seed (`sts2_rl::human`). Bosses 65%,
hallways 12% (acts 2-3, HP capped at 60%), elites the rest, acts 2-3 weighted 2x. A gym fight is
terminal: no run reward, the cell's `win_bonus` (V after a won fight, measured) when survived,
optional `hp_coef`; its rows train at `loss_weight` (0.6; `gym_mass` logs the run rows' share of
the weighted rows) and its deaths stay out of RLN37's counts. `practice.share` 1.0 is combat-only.
`--set practice='{"share":[0.5,0.5],"human":{"enabled":true}}'` at a resume.

**Exploration floors** (RLN43, `explore_floor`, None = off and bit-identical; `kind_log`, on by
default, only logs): `ExploreFloor`, `EXPLORE_KINDS`; `explore` in the log has per kind the batch
entropy fraction, rows, KL and P(option) (`p_smith` by HP band, `p_skip`, `p_remove`, `p_buy`,
`p_use`, `p_discard`), the coefficient and the kinds on.

**Warm start** (RLN42, `init_from`, fresh runs only): `--set init_from=<state.pt>
--set init_weights=ema0.999 --set init_reset=["run_head.","v_hidden.","v_out."]` loads that
checkpoint's net (raw or one of its EMAs) into the new one, except the parameters under the listed
prefixes, which keep this net's own init (a parameter the source lacks must be listed); the
optimiser, counters, RNG, practice state and EMAs start fresh (the EMAs at the loaded weights).
`run_head` (fresh runs only) gives the rows outside combat their own policy head (`E2ENet`).
**KL to the prior** (RLN42, `prior`, None = off and bit-identical; settable at a resume):
`--set prior={"path":...,"coef":0.1,"decay_to":0.02,"flat":2.5e8,"decay_steps":5e8}` adds
beta * KL(pi_M || prior) on run rows the RLN41 adapter maps (`rln42_prior` module doc); `prior` in
the log has the KL per kind, beta, and the collect's Smith, skip and removal rates and the gold on
entering a shop.

**Log** (`<out>/stats.jsonl`, one line per iteration): decisions/s (collect, update, total),
runs ended, mean floor, win rate, caps, KL, losses, entropy, explained variance per row type,
tokens and GPU memory; `rows_empty` and `empty_ends`: the rows with an
empty mask (RLN18: an episode that ended at its reset, e.g. a practice start whose fight is over
before its first decision; no decision, so they leave the update) and how those episodes ended.
With `STS2_COLLECT_TIMING=1` (RLN25), `collect_timing`: the collect's host seconds per phase
(`batch_in`, `forward`, `sample`, `wait` for `.cpu()`, the env `step`, `rest`, its `practice`
part) and the GPU's forward and sampling seconds; `STS2_STEP_TIMING=1` makes the env print its
threads' busy times to stderr (`sts2_rl::step_timing`).
"""

import argparse
import dataclasses
import json
import math
import os
import pathlib
import sys
import threading
import time

# Python.h for Triton's launcher build (see the docstring); harmless when absent.
_PYINC = pathlib.Path.home() / ".local" / "pyinclude"
if _PYINC.is_dir():
    os.environ["CPATH"] = os.pathsep.join(
        [p for p in (os.environ.get("CPATH"), str(_PYINC / "python3.14"), str(_PYINC)) if p])

# Variable token widths fragment the caching allocator; expandable segments keep the reserved
# memory near the allocated. torch reads it when it loads: `e2e_lane.py` sets it before that too.
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

import numpy as np  # noqa: E402
import torch  # noqa: E402
import torch._dynamo as dynamo  # noqa: E402
import torch.fx.experimental._config as fx_config  # noqa: E402

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import E2E, HOLD, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e import returns as R  # noqa: E402
from sts2.e2e.net import E2ENet, masked_stats  # noqa: E402
from sts2.env import GYM_END, PRACTICE_END, RUN_ENDS  # noqa: E402
from sts2.ffi import CELL_NAMES, GYM_CELL_NAMES, GYM_CODE, cell_name  # noqa: E402

TRAIN_SEEDS = (100_000_000, 200_000_000)
PRACTICE_SEED = np.uint64(2**64 - 1)  # `env_seeds` of an env playing a practice episode
# RLN25: `collect_timing` in the log (where the collect's wall time goes), off by default.
PRACTICE_CHUNK = 16384  # rows per bootstrap forward (RLN26)
COLLECT_TIMING = os.environ.get("STS2_COLLECT_TIMING") == "1"


@dataclasses.dataclass
class Reward:
    win: float = 1.0
    boss: float = 0.05
    floor: float = 0.002
    upgrade: float = 0.0  # RLN40: per upgrade level gained in the deck (0 = off, bit-identical)


DECK_AT, DECK_CARDS = 32, 80  # `run_obs.rs`: the deck's (card u16, upgrade u8, flags u8) entries


def deck_upgrades(obs):
    """Upgrade levels in the deck, per row of the run obs bytes (n, >= 352) (RLN40)."""
    d = np.asarray(obs)[:, DECK_AT:DECK_AT + 4 * DECK_CARDS].reshape(len(obs), DECK_CARDS, 4)
    present = (d[:, :, 0] != 0) | (d[:, :, 1] != 0)
    return (d[:, :, 2].astype(np.int64) * present).sum(1)


GYM_DEFAULT_PATH = "~/.cache/spirebot-data/rln43/gym.jsonl"


@dataclasses.dataclass
class Human:
    """RLN43 combat gym (`practice.human`, see the module docs): fights from strong human runs.

    `share` of the practice slots play human starts (the rest own snapshots); with the
    practice share at 1.0 every env does (combat-only). Of the gym's starts `boss` are bosses,
    `hall` hallway fights (HP capped at `hall_hp_pct`% of max HP), the rest elites; acts by
    `act_weights` (bosses, elites), `hall_acts` (hallways). A gym episode is terminal at the
    fight's end; its rows get no run reward (floors, bosses, win) but the cell's `win_bonus` when
    the fight is survived and `hp_coef` * (phi after - phi before), phi = HP / max HP.
    `loss_weight` scales the gym rows' policy, entropy and value loss.

    `win_bonus` (a float for every cell, or 9 by `GYM_CELL_NAMES`: elites, bosses, hallways by
    act) stands in for what a normal fight bootstraps into, V(the run after the fight): the
    default is NECRO5's mean V after a won human-panel fight (444M, 870 fights, sd 0.003-0.019),
    so a gym fight's terminal target matches a normal fight's in expectation and the value head
    needs no gym flag (a flat 0.05 was 4.7x too high for an act-3 boss). Re-measure it for
    another net (`docs/research/rln43-combat-gym.md`)."""
    enabled: bool = False
    path: str = GYM_DEFAULT_PATH
    share: float = 0.5
    boss: float = 0.65
    hall: float = 0.12
    act_weights: tuple = (1.0, 2.0, 2.0)
    hall_acts: tuple = (0.0, 1.0, 1.0)
    hall_hp_pct: int = 60
    variants: bool = False
    win_bonus: tuple = (0.066, 0.031, 0.014, 0.046, 0.027, 0.011, 0.072, 0.032, 0.018)
    hp_coef: float = 0.0
    loss_weight: float = 0.6
    # CB2: progress on a loss. `enemy_coef` * (dealt after - dealt before), dealt = the share of
    # the enemies' max HP gone (`enemy_dealt`: Osty excluded, 1 once the fight is won), so a lost
    # fight still pays for damage done and a won one adds `enemy_coef` in all (a loss never
    # scores above a win while `enemy_coef` <= `win_bonus`). `pot_coef` * (potions after -
    # before), 0 potions after a death (the MCTS utility's potion term). 0 = off, bit-identical.
    enemy_coef: float = 0.0
    pot_coef: float = 0.0

    @classmethod
    def from_dict(cls, d):
        d = dict(d or {})
        for k in ("act_weights", "hall_acts"):
            if k in d:
                d[k] = tuple(d[k])
        if isinstance(d.get("win_bonus"), (list, tuple)):
            d["win_bonus"] = tuple(float(x) for x in d["win_bonus"])
            if len(d["win_bonus"]) != len(GYM_CELL_NAMES):
                raise ValueError(f"practice.human.win_bonus: one float or {len(GYM_CELL_NAMES)}")
        return cls(**d)

    def bonus(self):
        """`win_bonus` per gym cell (an array by `GYM_CELL_NAMES`)."""
        b = self.win_bonus
        return np.full(len(GYM_CELL_NAMES), float(b)) if np.isscalar(b) else np.asarray(b, float)

    def weights(self):
        """Cell weights in parts per million, by `ffi.GYM_CELL_NAMES`."""
        def norm(w):
            t = float(sum(w))
            return [x / t for x in w] if t > 0 else [0.0] * 3
        elite = 1.0 - self.boss - self.hall
        w = [elite * x for x in norm(self.act_weights)]
        w += [self.boss * x for x in norm(self.act_weights)]
        w += [self.hall * x for x in norm(self.hall_acts)]
        return [round(1e6 * max(0.0, x)) for x in w]

    def split(self, slots, envs):
        """(own-snapshot slots, gym slots) out of `slots` practice slots."""
        if slots >= envs:
            return 0, envs
        gym = round(self.share * slots)
        return slots - gym, gym


@dataclasses.dataclass
class Practice:
    """Practice starts (see the module docs); `start` is set when practice is turned on."""
    enabled: bool = False
    start: int = None
    span: int = 2_000_000_000
    share: tuple = (0.40, 0.25)  # of the env slots, linear over `span`
    boss_share: tuple = (0.40, 0.75)  # of the starts
    boss_mix: tuple = (0.15, 0.20, 0.65)  # acts 1-3
    elite_mix: tuple = (0.20, 0.40, 0.40)
    pool: int = 32
    # RLN34: 16 and 2, not spire-mentor's 4 and 8: a scarce cell (the act-3 boss) gave at most 4
    # starts per capture, sat just under 8 entries and fell back to the act-2 boss (6% of starts
    # for 42% intended); 16/2 measured 23% (`python/rln34_act3.py --share`).
    max_uses: int = 16
    min_active: int = 2
    # RLN37 (arXiv 2611.12088): failure-driven weights, off by default (off: the fixed mix above,
    # bit-identical). On: the normal runs' deaths per cell (`DEATH_*`) feed L = decay * L + D
    # every `adaptive_every` iterations and the cell weights replace `elite_mix`/`boss_mix`/
    # `boss_share`: (1 - floor) * L / sum(L) + floor / 6 (the fixed mix until L is above 0).
    adaptive: bool = False
    adaptive_every: int = 10
    adaptive_decay: float = 0.5
    adaptive_floor: float = 0.1
    # RLN44: HP lost in practice fights (AlphaDefect counts it): each practice row's reward gets
    # c * (phi(after) - phi(before)), phi = HP / max HP (`hp_fraction`, 0 at a death), so a fight
    # adds -c * HP lost / max HP in all; a potential difference pays nothing for stalling.
    # 0 = off (nothing computed, bit-identical). Normal runs are unchanged.
    hp_coef: float = 0.0
    # RLN43: the combat gym (`Human`, a dict here so old checkpoints load); None = off.
    human: dict = None

    @classmethod
    def from_dict(cls, d):
        d = dict(d or {})
        for k in ("share", "boss_share", "boss_mix", "elite_mix"):
            if k in d:
                d[k] = tuple(d[k])
        if d.get("human") is not None:
            d["human"] = dataclasses.asdict(Human.from_dict(d["human"]))
        return cls(**d)

    def gym(self):
        """The gym's settings if it is on, else None."""
        h = Human.from_dict(self.human) if self.human else None
        return h if h is not None and h.enabled else None

    def adaptive_weights(self, loss):
        """The failure-driven cell weights (sum 1) from the decayed death counts `loss`, or None
        while there are none."""
        total = float(sum(loss))
        if total <= 0:
            return None
        f = self.adaptive_floor
        return [(1 - f) * v / total + f / len(loss) for v in loss]

    def schedule(self, decisions, envs, loss=None):
        """(slots, cell weights in parts per million) at `decisions`; `loss` is the adaptive L."""
        x = min(1.0, max(0.0, (decisions - (self.start or 0)) / self.span))
        lerp = lambda ab: ab[0] + (ab[1] - ab[0]) * x  # noqa: E731
        w = self.adaptive_weights(loss) if self.adaptive and loss is not None else None
        if w is not None:
            return round(lerp(self.share) * envs), [round(1e6 * v) for v in w]
        b = lerp(self.boss_share)
        weights = [round(1e6 * (1 - b) * m) for m in self.elite_mix]
        weights += [round(1e6 * b * m) for m in self.boss_mix]
        return round(lerp(self.share) * envs), weights

    def adaptive_step(self, loss, deaths):
        """L after one update: `adaptive_decay` * L + D."""
        return [self.adaptive_decay * a + b for a, b in zip(loss, deaths)]


HEADER_HP, COMBAT_HP = 8, E.COMBAT_AT  # `run_obs.rs` header and `obs.rs` header: HP, max HP (i16)


def hp_fraction(obs):
    """HP / max HP per row of the run obs bytes: the combat section's live HP during a fight,
    else the header's (RLN44 `hp_coef`)."""
    o = np.asarray(obs)

    def i16(at):
        return (o[:, at].astype(np.int64) | o[:, at + 1].astype(np.int64) << 8).astype(np.int16)
    c_hp, c_max = i16(COMBAT_HP), i16(COMBAT_HP + 2)
    fight = c_max > 0
    hp = np.where(fight, c_hp, i16(HEADER_HP)).astype(np.float64)
    mx = np.where(fight, c_max, i16(HEADER_HP + 2)).astype(np.float64)
    return np.clip(hp, 0, None) / np.maximum(mx, 1)


HEADER_PHASE, PHASE_COMBAT = 3, 3  # run header phase byte; `naz3_combat.H_PHASE`
BELT_AT, BELT_SLOTS = 480, 6  # `naz3_combat.belt_counts`
C_CREATURES, CREATURE_BYTES, CREATURES = 24, 60, 8  # `rln19_act2.py`
_OSTY = None


def belt_count(obs):
    """Potions on the belt per row of the run obs bytes (CB2 `pot_coef`)."""
    o = np.asarray(obs)
    b = o[:, BELT_AT:BELT_AT + 2 * BELT_SLOTS].reshape(len(o), BELT_SLOTS, 2)
    return (b != 0).any(2).sum(1).astype(np.float64)


def enemy_dealt(obs):
    """The share of the enemies' max HP gone (Osty excluded, dead enemies counted at their max
    HP) per row of the run obs bytes; 0 outside a fight (CB2 `enemy_coef`, as
    `naz3_combat.enemy_dealt`)."""
    global _OSTY
    if _OSTY is None:
        import rln19_act2

        _OSTY = rln19_act2._vocab("monsters").index("OSTY")
    o = np.asarray(obs)
    c = COMBAT_HP + C_CREATURES
    b = o[:, c:c + CREATURE_BYTES * CREATURES].reshape(len(o), CREATURES, CREATURE_BYTES)
    b = b.astype(np.int32)
    mon = b[:, :, 4] | b[:, :, 5] << 8
    hp = (b[:, :, 6] | b[:, :, 7] << 8).astype(np.int16)
    mx = (b[:, :, 8] | b[:, :, 9] << 8).astype(np.int16)
    enemy = (b[:, :, 1] == 1) & (mon != _OSTY) & ((mon != 0) | (b[:, :, 0] != 0))
    alive = enemy & (b[:, :, 0] != 0)
    ehp = np.where(alive, np.maximum(hp, 0), 0).sum(1)
    emax = np.where(enemy, mx, 0).sum(1)
    return np.where(emax > 0, 1.0 - ehp / np.maximum(emax, 1), 0.0)


DEATH_HEADER_ACT, DEATH_HEADER_ROOM, DEATH_ROOM_ELITE, DEATH_ROOM_BOSS = 2, 26, 2, 3


def death_cell(obs):
    """The practice cell (`CELL_NAMES` index) of a run's last observation (header: act index,
    `RoomType` index + 1 of its combat room), or None: a hallway fight or a non-combat room."""
    act, room = int(obs[DEATH_HEADER_ACT]), int(obs[DEATH_HEADER_ROOM])
    if act > 2 or room not in (DEATH_ROOM_ELITE, DEATH_ROOM_BOSS):
        return None
    return act + (3 if room == DEATH_ROOM_BOSS else 0)


RESUME_SETTABLE = ("compile", "threads", "sort_micro", "fused_adam", "micro", "cache_tokens",
                   "practice", "kl_source", "returns", "adv_norm", "obs_v2", "collect_buckets",
                   "micro_width", "lr_factor_range", "ent_boost", "ent_boost_floor",
                   "ent_boost_iters", "ent_boost_max", "reward", "prior", "behaviour",
                   "held_max", "fast_gather", "fixed_probe", "epochs", "e1_at",
                   "static_head", "compile_encode", "pinned_copies", "compile_net",
                   "explore_floor", "kind_log", "no_full_heal")
"""What `--set` may change at a resume: the speed flags, practice (a dict merged into the
checkpoint's), `kl_source`, the returns and advantage modes (RLN15), `obs_v2` (RLN16: turning
it on upgrades a v1 checkpoint; its new weights start at zero, so the net's outputs are unchanged
at the swap) and `lr_factor_range` (RLN34: the saved factor is clamped into the new range) and the entropy collapse rule
(RLN35: `ent_boost` and its parameters) and `reward` (RLN40, merged: `--set reward='{"upgrade": 0.001}'`)."""


def practice_ends(out, done, ended, end, won, bosses):
    """Tallies the practice episodes that ended (`done` and `ended` > 0, the cell code + 1) into
    `out` (`Learner._practice_ends`): per cell [ended, survived], `out["ends"][cell][end name]`
    and, for a boss cell, `out["kills"][cell]` [0, 1, 2+ bosses killed]. Survived: truncated,
    won, or a boss cell that killed a boss (an act-3 boss start plays on to the run's end)."""
    ends, kills = out.setdefault("ends", {}), out.setdefault("kills", {})
    for i in np.nonzero(done & (ended > 0))[0]:
        cell = cell_name(int(ended[i]))
        boss = cell.startswith("BOSS")
        rec = out.setdefault(cell, [0, 0])
        rec[0] += 1
        rec[1] += int(end[i] in (PRACTICE_END, GYM_END) or bool(won[i])
                      or (boss and bosses[i] > 0))
        name = RUN_ENDS[end[i]]
        e = ends.setdefault(cell, {})
        e[name] = e.get(name, 0) + 1
        if boss:
            kills.setdefault(cell, [0, 0, 0])[min(int(bosses[i]), 2)] += 1


class FamilyEntropy:
    """The update's mean entropy per family (combat, run): sums and row counts over every micro
    of every minibatch and epoch (RLN34). Averaging each micro's family mean by its row share
    added 0 for a micro with no row of the family (token-budget micros, `Learner._micros`)."""

    def __init__(self, dev):
        self.acc = torch.zeros(4, dtype=torch.float64, device=dev)  # sum_c n_c sum_r n_r

    def add(self, ent, combat):
        cf = combat.double()
        e = ent.detach().double()
        self.acc += torch.stack([(e * cf).sum(), cf.sum(), (e * (1 - cf)).sum(), (1 - cf).sum()])

    def means(self):
        s_c, n_c, s_r, n_r = self.acc.tolist()
        return s_c / max(n_c, 1.0), s_r / max(n_r, 1.0)


def set_resumed(cfg, k, v):
    """`--set k=v` on a resumed `cfg` (`RESUME_SETTABLE`)."""
    if k == "kl_source" and v not in ("fixed", "batch"):
        raise ValueError(f"kl_source must be 'fixed' or 'batch', not {v!r}")
    if k == "practice":
        v = dict(v)
        if isinstance(v.get("human"), dict):  # RLN43: merged into the saved gym settings
            v["human"] = {**(cfg.practice.human or {}), **v["human"]}
        cfg.practice = Practice.from_dict({**dataclasses.asdict(cfg.practice), **v})
    elif k == "reward":  # RLN40: a dict merged into the saved one (`{"upgrade": 0.001}`)
        cfg.reward = Reward(**{**dataclasses.asdict(cfg.reward), **v})
    elif k == "prior":  # RLN42: a dict merged into the saved one; None turns it off
        cfg.prior = None if v is None else {**(cfg.prior or {}), **v}
    elif k == "explore_floor":  # RLN43: a dict merged into the saved one; None turns it off
        cfg.explore_floor = None if v is None else {**(cfg.explore_floor or {}), **v}
        if cfg.explore_floor is not None:
            ExploreFloor(**cfg.explore_floor)  # validates
    elif k == "ent_boost_floor":
        c, r = (float(x) for x in v)
        cfg.ent_boost_floor = (c, r)
    elif k == "lr_factor_range":
        lo, hi = (float(x) for x in v)
        if not 0 < lo <= hi:
            raise ValueError(f"lr_factor_range must be 0 < lo <= hi, not {v!r}")
        cfg.lr_factor_range = (lo, hi)
    else:
        setattr(cfg, k, v)


REST_ACTIONS = slice(236, 244)  # the rest site's option slots in the run action space
REST_BANDS = (0.3, 0.5, 0.7, 0.9)  # P(Smith) by HP band: <30%, 30-49, 50-69, 70-89, >=90%
REST_SMITH = 1  # the rest option kind of Smith (`rln40_smith.KIND_SMITH`)


EXPLORE_KINDS = {
    # kind: (single-option entity kinds, grouped options, a remainder option, named groups)
    "rest": (("rest_option",), (), False, {}),
    "card_reward": (("reward_card", "reward_skip", "reward_alternative"), (), False,
                    {"skip": ("reward_skip",)}),
    "removal": ((), (("shop_remove",),), True, {"remove": ("shop_remove",)}),
    "shop": (("shop_card", "shop_relic", "shop_potion"), (), True,
             {"buy": ("shop_card", "shop_relic", "shop_potion")}),
    "potion": ((), (("belt_potion",), ("discard_potion",)), True,
               {"use": ("belt_potion",), "discard": ("discard_potion",)}),
    "map": (("map_column",), (), False, {}),
}
"""The run-decision kinds (outside fights) by the action table's entity kinds. A kind's options
on a row: each legal action of a single-option entity kind; each group (any legal action in it);
with a remainder, every other legal action together as one option ("not this": no removal, no
purchase, no potion). Without a remainder the policy is renormalised over the kind's actions.
A row has the kind when >= 2 of its options are legal."""


@dataclasses.dataclass
class ExploreFloor:
    """Entropy floors per run-decision kind (`explore_floor`, off by default). Each iteration
    the update measures every kind's batch entropy fraction (the mean over its rows of
    H(options) / log(#legal options), first epoch); a kind whose last measurement is below its
    floor gets + c(d) * KL(its options' policy, uniform over the legal options) on its rows, and
    none at or above it (so it switches on and off; at a start or resume it is off until the
    first measurement). c(d) = `coef` * max(0, 1 - (d - `start`) / `decay_steps`), `start` the
    decision count when it was turned on. No kind is pulled toward a particular option.

    `direction` "pi_u" is KL(pi || U) = log K - H(pi), the entropy bonus's form: its gradient on
    a collapsed option's logit is ~ c * q * |log q| and vanishes as q -> 0. "u_pi" is
    KL(U || pi) = -mean log pi - log K, gradient c * (1/K - q), which lifts a collapsed option."""

    floors: dict = dataclasses.field(default_factory=dict)
    coef: float = 0.05
    decay_steps: int = 100_000_000
    direction: str = "pi_u"
    target: str = "uniform"
    start: int = None

    def __post_init__(self):
        bad = set(self.floors) - set(EXPLORE_KINDS)
        if bad:
            raise ValueError(f"explore_floor: unknown kinds {sorted(bad)}; {list(EXPLORE_KINDS)}")
        if self.target != "uniform":
            raise ValueError(f"explore_floor.target {self.target!r}: only 'uniform'")
        if self.direction not in ("pi_u", "u_pi"):
            raise ValueError(f"explore_floor.direction {self.direction!r}: 'pi_u' or 'u_pi'")
        if self.coef < 0 or self.decay_steps <= 0:
            raise ValueError("explore_floor: coef >= 0 and decay_steps > 0")

    def coef_at(self, decisions):
        done = max(0, decisions - (self.start or 0))
        return self.coef * max(0.0, 1.0 - done / self.decay_steps)

    def active(self, ent):
        """The kinds on, from the last measured entropy fractions `ent` ({kind: frac|None})."""
        return {k for k, f in self.floors.items() if ent.get(k) is not None and ent[k] < f}


class KindTables:
    """Per kind: each action's option index (-1: not the kind's), the option count, and the
    named groups' action masks, on `device`."""

    def __init__(self, device):
        from sts2.env import ENTITY_KINDS, action_table
        ek = action_table()[:, 0].astype(np.int64)
        cols = lambda names: np.isin(ek, [ENTITY_KINDS.index(x) for x in names])  # noqa: E731
        self.opt, self.n, self.named = {}, {}, {}
        for kind, (singles, groups, remainder, named) in EXPLORE_KINDS.items():
            opt = np.full(len(ek), -1, np.int64)
            single = np.flatnonzero(cols(singles))
            opt[single] = np.arange(len(single))
            m = len(single)
            for g in groups:
                opt[cols(g)] = m
                m += 1
            if remainder:
                opt[opt < 0] = m
                m += 1
            self.opt[kind] = torch.from_numpy(opt).to(device)
            self.n[kind] = m
            self.named[kind] = {nm: torch.from_numpy(cols(g)).to(device) for nm, g in named.items()}

    def stats(self, kind, logp_all, run):
        """Per row of `logp_all` (B, actions; -inf illegal): whether it has `kind` (and `run`),
        the option log-probs (B, M; renormalised; 0 where illegal), the legal options, their
        count, and the entropy fraction (0 off the kind's rows)."""
        cols = self._cols(kind, logp_all.shape[1])
        o = self.opt[kind][cols]
        lp = logp_all[:, cols]
        legal_c = torch.isfinite(lp)
        p = torch.where(legal_c, lp.exp(), torch.zeros_like(lp))
        m = self.n[kind]
        mass = torch.zeros(lp.shape[0], m, device=lp.device, dtype=p.dtype).index_add_(1, o, p)
        legal = torch.zeros(lp.shape[0], m, device=lp.device).index_add_(
            1, o, legal_c.float()) > 0
        n = legal.sum(-1)
        on = run & (n >= 2)
        lq = torch.log(mass.clamp(min=1e-30)) - torch.log(mass.sum(-1, keepdim=True).clamp(
            min=1e-30))
        lq = torch.where(legal, lq, torch.zeros_like(lq))
        h = -(lq.exp() * lq * legal).sum(-1)
        frac = torch.where(on, h / torch.log(n.clamp(min=2).float()), torch.zeros_like(h))
        return on, lq, legal, n, frac

    def _cols(self, kind, width):
        """The kind's action indices below `width` (355 without `discard_potion`'s slots)."""
        c = self.__dict__.setdefault("_colcache", {})
        if (kind, width) not in c:
            c[kind, width] = torch.nonzero(self.opt[kind][:width] >= 0).squeeze(1)
        return c[kind, width]

    def named_p(self, kind, logp_all, on):
        """{name: (sum of P(group) over the kind's rows where the group is legal, rows)}, P
        renormalised over the kind's actions."""
        w = logp_all.shape[1]
        cols = self._cols(kind, w)
        p = torch.where(torch.isfinite(logp_all), logp_all.exp(), torch.zeros_like(logp_all))
        tot = p[:, cols].sum(-1).clamp(min=1e-30)
        out = {}
        for nm, mask in self.named[kind].items():
            mask = mask[:w]
            g = (p * mask).sum(-1) / tot
            has = on & (torch.isfinite(logp_all) & mask).any(-1)
            out[nm] = ((g * has).sum(), has.sum())
        return out


def explore_kl(lq, legal, n, on, direction):
    """Per row: KL(options' policy, uniform over the legal options), 0 off `on`."""
    k = n.clamp(min=2).float()
    if direction == "pi_u":
        kl = (lq.exp() * lq * legal).sum(-1) + torch.log(k)
    else:
        kl = -(lq * legal).sum(-1) / k - torch.log(k)
    return torch.where(on, kl, torch.zeros_like(kl))


def rest_rows(obs):
    """Per row of the run obs bytes: -2 not a rest site, else Smith's option slot (-1: not
    offered or not enabled); and the HP band (`REST_BANDS`)."""
    o = np.asarray(obs)
    rest = o[:, 3] == 7
    k = o[:, E.REST_AT:E.REST_AT + 16].reshape(len(o), 8, 2)
    smith = (k[:, :, 0].astype(np.int64) - 1 == REST_SMITH) & (k[:, :, 1] > 0)
    slot = np.where(smith.any(1), smith.argmax(1), -1)
    band = np.searchsorted(REST_BANDS, hp_fraction(o), side="right")
    return np.where(rest, slot, -2).astype(np.int8), band.astype(np.uint8)


@dataclasses.dataclass
class Config:
    envs: int = 2048
    threads: int = 14
    steps: int = 64
    minibatch: int = 8192
    micro: int = 2048  # rows per forward/backward inside a minibatch (GPU memory)
    epochs: int = 2
    e1_at: int = 250_000_000
    clip: float = 0.2
    dual_clip: float = 3.0
    vf_coef: float = 1.0
    lr: float = 1e-4
    value_lr: float = 3e-4
    weight_decay: float = 1e-4
    grad_clip: float = 1.0
    warmup_iters: int = 10
    lam_combat: float = 0.95
    lam_run: float = 0.97
    ent_combat: tuple = (0.01, 0.002)
    ent_run: tuple = (0.0125, 0.006)
    ent_flat: int = 100_000_000
    ent_end: int = 4_000_000_000
    ent_boost: bool = False  # RLN35: the entropy collapse rule (module docs)
    ent_boost_floor: tuple = (0.10, 0.05)  # (combat, run) normalised entropy
    ent_boost_iters: int = 20
    ent_boost_max: int = 2
    probe_rows: int = 8192
    kl_source: str = "fixed"  # RLN14: which KL drives the lr factor and the early stop: "fixed"|"batch"
    kl_low: float = 0.005
    kl_high: float = 0.02
    kl_stop: float = 0.06
    lr_factor_range: tuple = (0.3, 2.0)
    ema: tuple = (0.996, 0.999)
    reward: Reward = dataclasses.field(default_factory=Reward)
    seed_range: tuple = TRAIN_SEEDS
    ascension: int = 10
    character: str = "Necrobinder"
    seed: int = 0
    layers: int = 4
    d: int = 256
    ckpt_every: int = 5_000_000
    device: str = "cuda"
    deterministic: bool = False
    # Speed (RLN9). Each is the same maths; all off is bit-identical to the RLN5 learner.
    overlap: bool = False  # collect batch N+1 with the pre-update weights during update N
    # (off: measured only 6-10% faster at full size, and its 15-min LC learned slower per
    # decision: the lagged batches' KL per update ran 2-4x higher and pinned the lr factor at 0.3)
    sort_micro: bool = True  # split each minibatch into micros of similar token width
    cache_tokens: bool = True  # keep the collect's tokens (packed) instead of re-encoding
    value_first: bool = True  # the value block computes only its read position
    fused_adam: bool = True
    compile: bool = False  # torch.compile the transformer blocks (needs the headers; else eager)
    probe_chunk: int = 2048  # rows per probe forward (0: all in one, as RLN5); memory only
    practice: Practice = dataclasses.field(default_factory=Practice)
    # Returns and advantages (RLN15, `sts2.e2e.returns`). "chained" and "global" are the
    # RLN5 maths; "two_chain" is spire-mentor's combat and run chains with run rows held back
    # up to `hold_iters` iterations; "per_type" their per-decision-type advantage moments.
    returns: str = "chained"
    adv_norm: str = "global"
    hold_iters: int = R.HOLD_ITERS
    # RLN42: at most this many held run rows; above it this iteration holds for fewer iterations
    # (NECRO4's rest -> Smith -> cancel loop held 330k rows and ran out of GPU, then host, memory).
    held_max: int = 120_000
    adv_decay: float = 5e-4
    adv_min_rows: int = R.ADV_MIN_ROWS
    obs_v2: bool = False  # RLN16: the obs v2 inputs (intent links, forecast, previews, routes)
    # RLN29, fresh runs only (not settable at a resume); all off is the same learner as before.
    card_stats: bool = False  # the content prior per card, relic and potion (`E2ENet` doc)
    emb_std: float = None  # the input embeddings' init std (None: 0.02)
    v_warm: int = 0  # the first N updates train the value module only (policy lr 0)
    # RLN30, fresh runs only: the full view (pile cards, the whole map, past moves; `E2ENet` doc)
    obs_full: bool = False
    # RLN31 (speed; with `obs_full` only, so NECRO1/NECRO2 stay bit-identical): the collect's
    # forwards run the rows in this many groups of similar token width (each cut to its longest).
    collect_buckets: int = 3
    # RLN31 (memory; with `obs_full` only): a micro holds at most `micro * micro_width` tokens
    # (rows times their longest), so the widest rows come in smaller micros and the narrow ones
    # in larger; 0: `micro` rows each.
    micro_width: int = 40
    # RLN32, env options only (the net is the same): discard-potion actions, the plan column's depth
    discard_potion: bool = False
    plan_depth: int = 1
    # RLN47, env option (`Sts2RunEnvs.no_full_heal`): no Heal at a rest site at full HP
    no_full_heal: bool = False
    # RLN33, fresh runs only: each event/ancient option's shown effects (`E2ENet` doc)
    obs_events: bool = False
    # RLN42, fresh runs only: the run rows' own head and the warm start (module docs)
    run_head: bool = False
    init_from: str = None
    init_weights: str = "ema0.999"
    init_reset: tuple = ()
    # RLN42: KL to the RLN41 prior on run decisions (`rln42_prior.PriorCfg` fields; None: off)
    prior: dict = None
    # RLN43: entropy floors per run-decision kind (`ExploreFloor` fields; None: off,
    # bit-identical) and the per-kind entropy / P(option) log (no effect on the training)
    explore_floor: dict = None
    kind_log: bool = True
    behaviour: bool = False  # RLN42: behaviour telemetry in the log (`rln42_watch`)
    # RLN44 speed flags (each the same maths to float noise; all off is bit-identical).
    fast_gather: bool = False  # `E2ENet.fast_gather`: the content-stats gathers by F.embedding
    # With `kl_source="batch"` the fixed probe only logs `kl`; False skips its two passes per
    # update (logged as NaN). The maths is unchanged: the batch probe drives the lr and the stop.
    fixed_probe: bool = True
    # The policy head's (row, action) pairs sized on the host (`encode.legal_counts`) and the run
    # head by `torch.where`: no device sync per forward (`E2ENet.head`).
    static_head: bool = False
    # torch.compile of `encode` for the collect's batches (`envs` rows; other sizes stay eager),
    # fixed shapes: ~2,400 small kernels per step become a few dozen.
    compile_encode: bool = False
    # The collect's host -> device copies from pinned memory, non_blocking (a pageable copy
    # waits for the GPU's queue: a hidden sync per array per step).
    pinned_copies: bool = False
    # With `compile` and `static_head`: the whole net (`forward`, `policy`: embeddings, blocks,
    # head, value) compiled as one graph (dynamic shapes) instead of the blocks alone.
    compile_net: bool = False

    @classmethod
    def from_dict(cls, d):
        d = dict(d)
        d["reward"] = Reward(**d.get("reward", {}))
        d["practice"] = Practice.from_dict(d.get("practice"))
        for k in ("ent_combat", "ent_run", "ent_boost_floor", "lr_factor_range", "ema", "seed_range",
                  "init_reset"):
            if k in d:
                d[k] = tuple(d[k])
        return cls(**d)


def gae(rewards, values, dones, combat, last_value, lam_combat, lam_run, boot=None):
    """Chained GAE, γ = 1: arrays (S, n) in step order; `dones[t]` ends the run after row t.
    λ is row t's (combat or run). `boot[t]` (a truncated practice episode's last row) is the
    value of the position after it, added to the row's reward. Returns (advantages, returns)."""
    steps = rewards.shape[0]
    adv = torch.zeros_like(values)
    next_adv = torch.zeros_like(last_value)
    next_value = last_value
    lam = torch.where(combat, torch.full_like(values, lam_combat),
                      torch.full_like(values, lam_run))
    for t in range(steps - 1, -1, -1):
        live = 1.0 - dones[t].float()
        delta = rewards[t] + live * next_value - values[t]
        if boot is not None:
            delta = delta + boot[t]
        next_adv = delta + lam[t] * live * next_adv
        adv[t] = next_adv
        next_value = values[t]
    return adv, adv + values


def ent_boost_step(cfg, ents, mult, streak, boosts):
    """RLN35 collapse rule after one iteration: `ents` the (combat, run) entropies; updates the
    lists `mult`, `streak`, `boosts` in place."""
    for i, (e, floor) in enumerate(zip(ents, cfg.ent_boost_floor)):
        if boosts[i] >= cfg.ent_boost_max:
            continue
        streak[i] = streak[i] + 1 if e < floor else 0
        if streak[i] >= cfg.ent_boost_iters:
            mult[i] *= 1.5
            boosts[i] += 1
            streak[i] = 0


def entropy_coef(cfg, decisions):
    """(combat, run) coefficients at `decisions`."""
    out = []
    for hi, lo in (cfg.ent_combat, cfg.ent_run):
        if decisions <= cfg.ent_flat:
            out.append(hi)
        else:
            f = min(1.0, (decisions - cfg.ent_flat) / (cfg.ent_end - cfg.ent_flat))
            out.append(hi * (lo / hi) ** f)
    return out


def _guarded(compiled, eager, name):
    """RLN45: `compiled`, except a call whose graph inductor fails to build runs `eager` (the
    same maths); the first such failure is logged with the call's shapes. A shape that compiles
    keeps its compiled graph."""
    seen = [0]

    def call(*args, **kw):
        try:
            return compiled(*args, **kw)
        except (dynamo.exc.BackendCompilerFailed, dynamo.exc.Unsupported) as e:
            if not seen[0]:
                shapes = [{k: tuple(v.shape) if torch.is_tensor(v) else v
                           for k, v in a.items()} if isinstance(a, dict) else a for a in args]
                print(f"compile_net: {name} graph failed, running eager "
                      f"({type(e).__name__}: {str(e)[:300]}); shapes {shapes}", flush=True)
            seen[0] += 1
            return eager(*args, **kw)

    call.eager_calls = seen
    return call


def net_of(cfg):
    """A fresh `E2ENet` for `cfg` (a `Config` or its dict)."""
    get = cfg.get if isinstance(cfg, dict) else lambda k, d=None: getattr(cfg, k, d)
    return E2ENet(d=get("d"), layers=get("layers"), obs_v2=get("obs_v2", False),
                  card_stats=get("card_stats", False), emb_std=get("emb_std"),
                  obs_full=get("obs_full", False), obs_events=get("obs_events", False),
                  discard_potion=get("discard_potion", False), run_head=get("run_head", False))


def warm_start(net, cfg):
    """RLN42: `cfg.init_from`'s weights (`init_weights`: raw or an EMA) into `net`, except the
    parameters under `init_reset`'s prefixes (they keep `net`'s init). Every other parameter and
    buffer must be in the source with the same shape. Returns {"kept": [...], "reset": [...]}."""
    s = torch.load(pathlib.Path(cfg.init_from).expanduser(), map_location="cpu",
                   weights_only=False)
    scfg = Config.from_dict(s["config"])
    src = net_of(scfg)
    src.load_net(s["net"])
    if cfg.init_weights != "raw":
        decay = float(cfg.init_weights.removeprefix("ema"))
        match = [i for i, d in enumerate(scfg.ema) if abs(d - decay) < 1e-12]
        if not match:
            raise ValueError(f"init_weights {cfg.init_weights}: the source has EMAs {scfg.ema}")
        with torch.no_grad():
            for p, t in zip(src.parameters(), s["emas"][match[0]]):
                p.copy_(t)
    have, want = src.state_dict(), net.state_dict()
    reset = tuple(cfg.init_reset)
    kept = [k for k in want if not k.startswith(reset)]
    bad = [k for k in kept if k not in have or have[k].shape != want[k].shape]
    extra = [k for k in have if k not in want]
    if bad or extra:
        raise ValueError(f"init_from {cfg.init_from}: missing or reshaped {bad} (list them in "
                         f"init_reset), not in this net {extra}")
    net.load_state_dict({**want, **{k: have[k] for k in kept}})
    return dict(kept=kept, reset=[k for k in want if k.startswith(reset)])


def explained_variance(pred, target):
    if target.numel() < 2:
        return float("nan")
    var = target.var()
    return float(1 - (target - pred).var() / var) if var > 0 else float("nan")


class Learner:
    def __init__(self, cfg, out, resume=False):
        self.cfg = cfg
        self.out = pathlib.Path(out)
        self.out.mkdir(parents=True, exist_ok=True)
        self.dev = torch.device(cfg.device)
        if cfg.deterministic:
            torch.use_deterministic_algorithms(True)
        torch.manual_seed(cfg.seed)
        self.net = net_of(cfg).to(self.dev)
        self.net.value_first = cfg.value_first
        self.net.fast_gather = cfg.fast_gather
        self.opt = self._optimizer(v2_group=cfg.obs_v2)
        self.emas = [[p.detach().clone() for p in self.net.parameters()] for _ in cfg.ema]
        self.gen = torch.Generator(device=self.dev)
        self.gen.manual_seed(cfg.seed + 1)
        # Overlap: the next batch is collected by `behaviour` (the weights before this
        # update) on its own thread, stream and sampling generator.
        self.behaviour = None
        self.pending = None  # (buf, runs, seconds, the position before it was collected)
        if cfg.overlap:
            self.behaviour = net_of(cfg).to(self.dev)
            self.behaviour.value_first = cfg.value_first
            self.behaviour.fast_gather = cfg.fast_gather
            self.behaviour.requires_grad_(False)
            self._copy_behaviour()
            self.cgen = torch.Generator(device=self.dev)
            self.cgen.manual_seed(cfg.seed + 2)
        self.cstream = (torch.cuda.Stream(priority=-1)
                        if cfg.overlap and self.dev.type == "cuda" else None)
        self.compiled = False  # set by the first update's dry run
        self._cencode = None  # RLN44 `compile_encode`
        self._nleg = None  # RLN44 `static_head`: the update's legal counts per row (host)
        E._action_tables(self.dev)  # cached on the default stream before any thread uses them
        E._table_cols(self.dev)
        E._priv_kind(self.dev)
        self.iteration = 0
        self.decisions = 0
        self.lr_factor = 1.0
        self.ent_mult, self.ent_streak, self.ent_boosts = [1.0, 1.0], [0, 0], [0, 0]
        self.adapt_loss, self.adapt_deaths = [0.0] * 6, [0] * 6  # RLN37: L and the window's D
        self.probe = None
        self.advnorm = R.AdvNorm(R.N_TYPES, cfg.adv_decay, cfg.adv_min_rows)
        self._hold_reset()
        self.floors = []  # every finished run's floor, in finishing order
        self.wins = []
        n = cfg.envs
        start = cfg.seed_range[0]
        self.env_seeds = np.arange(start, start + n, dtype=np.uint64)
        self.next_serial = n
        self.run_start = np.zeros(n, dtype=np.int64)  # global step at which each run began
        self.ep_bosses = np.zeros(n, dtype=np.int64)  # bosses killed this episode (practice log)
        self.step_count = 0
        self.action_log = []  # (n,) uint16 per global step since min(run_start)
        self.log_base = 0
        self.envs = Sts2RunEnvs(n, seed_range=cfg.seed_range, character=cfg.character,
                                ascension=cfg.ascension, combat="agent", threads=cfg.threads,
                                priv=True, obs_v2=cfg.obs_v2, obs_full=cfg.obs_full,
                                discard_potion=cfg.discard_potion, plan_depth=cfg.plan_depth,
                                obs_events=cfg.obs_events, no_full_heal=cfg.no_full_heal,
                                **E2E)
        E.check_layout(self.envs.layout, v2=cfg.obs_v2, full=cfg.obs_full,
                       events=cfg.obs_events)
        self.obs_width = E.obs_width(cfg.obs_v2, cfg.obs_full, cfg.obs_events)
        if cfg.practice.enabled and not self.envs.has_practice:
            raise RuntimeError("practice needs an ABI-7 sts2-ffi library: rebuild it")
        if resume:
            self.load(self.out / "state.pt")
        elif cfg.init_from:  # RLN42: the net from a checkpoint; EMAs at it, the rest fresh
            self.warm = warm_start(self.net, cfg)
            self.emas = [[p.detach().clone() for p in self.net.parameters()] for _ in cfg.ema]
        self.prior = None
        if cfg.prior is not None:  # RLN42
            import rln42_prior
            self.prior = rln42_prior.PriorKL(rln42_prior.PriorCfg.from_dict(cfg.prior), self.dev)
        self.telemetry = None  # (`behaviour` is the overlap's collect net)
        if cfg.behaviour:  # RLN42: host-side counters only; the training does not change
            import rln42_watch
            self.telemetry = rln42_watch.Behaviour(n)
        if self.prior is not None or self.telemetry is not None:
            import rln42_prior
            self.shops = rln42_prior.ShopEntries(n)
        if cfg.practice.enabled and cfg.practice.start is None:
            cfg.practice.start = self.decisions
        self.explore, self.kinds, self.kind_ent = None, None, {}
        if cfg.explore_floor is not None:  # RLN43: the decay counts from this resume
            if cfg.explore_floor.get("start") is None:
                cfg.explore_floor = {**cfg.explore_floor, "start": self.decisions}
            self.explore = ExploreFloor(**cfg.explore_floor)
        if cfg.kind_log or self.explore is not None:
            self.kinds = KindTables(self.dev)
        self.gym_loaded = False
        self._practice_apply()

    def _hold_reset(self):
        """No held rows; every env's current run gets a fresh uid (a start or a resume:
        theirs drops the held rows at a restart, `HoldBack.resume`)."""
        n = self.cfg.envs
        self.held, self.held_in = {}, None
        self.run_uid = np.arange(n, dtype=np.int64)
        self.next_uid = n
        self.row_t = 0  # the table's `t` of the next step (all envs step together)
    def _optimizer(self, v2_group):
        """AdamW over [policy, value] (+ the v2 inputs, RLN21's side inputs and RLN22's plan
        column, a third, fourth and fifth group at the policy's lr)."""
        cfg = self.cfg
        value_ids = {id(p) for p in self.net.value_parameters()}
        v2_ids = ({id(p) for p in self.net.v2_parameters() + self.net.side_parameters()
                   + self.net.plan_parameters()} if cfg.obs_v2 else set())
        groups = [{"params": [p for p in self.net.policy_parameters() if id(p) not in v2_ids],
                   "lr": cfg.lr, "base": cfg.lr},
                  {"params": [p for p in self.net.parameters() if id(p) in value_ids],
                   "lr": cfg.value_lr, "base": cfg.value_lr}]
        if v2_group:
            groups += [self._v2_group(), self._side_group(), self._plan_group()]
        return torch.optim.AdamW(groups, weight_decay=cfg.weight_decay, eps=1e-8,
                                 fused=True if cfg.fused_adam else None)

    def _v2_group(self):
        return {"params": self.net.v2_parameters(), "lr": self.cfg.lr, "base": self.cfg.lr}

    def _side_group(self):
        """RLN21's inputs (`x_side`): a fourth group at the policy's lr, so an ABI-8 v2
        checkpoint's three groups load unchanged."""
        return {"params": self.net.side_parameters(), "lr": self.cfg.lr, "base": self.cfg.lr}

    def _plan_group(self):
        """RLN22's plan column (`x_plan`): a fifth group at the policy's lr, so an ABI-9
        checkpoint's four groups load unchanged."""
        return {"params": self.net.plan_parameters(), "lr": self.cfg.lr, "base": self.cfg.lr}

    def _later_groups(self):
        """The groups after the policy and value ones, in order: (net prefix that marks a
        checkpoint with them, group, its parameters)."""
        n = self.net
        return [(E2ENet.V2_PREFIX, self._v2_group, n.v2_parameters),
                (E2ENet.SIDE_PREFIX, self._side_group, n.side_parameters),
                (E2ENet.PLAN_PREFIX, self._plan_group, n.plan_parameters)]

    def _copy_behaviour(self):
        with torch.no_grad():
            torch._foreach_copy_(list(self.behaviour.parameters()),
                                 [p.detach() for p in self.net.parameters()])

    def _practice_apply(self):
        """The practice schedule at the current decision count, to the envs (between collects:
        never while a collect thread steps them)."""
        pr = self.cfg.practice
        if not pr.enabled:
            return
        slots, weights = pr.schedule(self.decisions, self.cfg.envs, self.adapt_loss)
        gym = pr.gym()
        gym_slots = 0
        if gym is not None:  # RLN43: envs `own..own + gym_slots` play human starts
            slots, gym_slots = gym.split(slots, self.cfg.envs)
            if not getattr(self, "gym_loaded", False):
                path = pathlib.Path(os.path.expanduser(gym.path))
                print(f"gym: {self.envs.load_gym(path)} human starts from {path}", flush=True)
                self.gym_loaded = True
        self.envs.set_practice(slots=slots, weights=weights, pool=pr.pool,
                               max_uses=pr.max_uses, min_active=pr.min_active, capture=True,
                               seed=self.cfg.seed)
        if gym is not None:
            self.envs.set_gym(first=slots, slots=gym_slots, weights=gym.weights(),
                              variants=gym.variants, hall_hp_pct=gym.hall_hp_pct,
                              seed=self.cfg.seed)
        elif getattr(self, "gym_loaded", False):
            self.envs.set_gym(off=True)

    # ---- env position, checkpoints ----

    def _track(self, actions, done):
        self.action_log.append(actions.astype(np.uint16))
        self.step_count += 1
        ended = np.nonzero(done)[0]
        cells = self.envs.cell
        for i in ended:  # the env hands out episodes in env order
            if cells is not None and cells[i]:  # a practice episode takes no serial
                self.env_seeds[i] = PRACTICE_SEED
            else:
                self.env_seeds[i] = self.cfg.seed_range[0] + self.next_serial
                self.next_serial += 1
            self.run_start[i] = self.step_count
        low = int(self.run_start.min())
        if low > self.log_base:
            del self.action_log[: low - self.log_base]
            self.log_base = low

    def _position(self):
        """The envs' position and the run records, as `save` writes them."""
        return dict(seeds=self.env_seeds.copy(), next_serial=self.next_serial,
                    run_start=self.run_start.copy(), step_count=self.step_count,
                    log_base=self.log_base, log=list(self.action_log),
                    floors=len(self.floors), wins=len(self.wins),
                    cgen=self.cgen.get_state() if self.cfg.overlap else None)

    def save(self, path=None):
        """With overlap, the batch collected ahead is not saved: the position, the sampling
        generator and the runs are those from before it, and the behaviour weights that
        collected it are; a resume collects it again, the same bits."""
        path = pathlib.Path(path or self.out / "state.pt")
        pos = self.pending[3] if self.pending is not None else self._position()
        state = dict(
            config=dataclasses.asdict(self.cfg),
            net=self.net.state_dict(),
            opt=self.opt.state_dict(),
            emas=[[t.cpu() for t in e] for e in self.emas],
            rng=dict(torch=torch.get_rng_state(), gen=self.gen.get_state(),
                     cuda=torch.cuda.get_rng_state_all() if torch.cuda.is_available() else None,
                     cgen=pos["cgen"]),
            iteration=self.iteration, decisions=self.decisions, lr_factor=self.lr_factor,
            ent_boost=dict(mult=list(self.ent_mult), streak=list(self.ent_streak),
                           boosts=list(self.ent_boosts)),
            adaptive=dict(loss=list(self.adapt_loss), deaths=list(self.adapt_deaths)),
            probe=None if self.probe is None else {k: v.cpu() for k, v in self.probe.items()},
            floors=np.asarray(self.floors[:pos["floors"]], dtype=np.int16),
            wins=np.asarray(self.wins[:pos["wins"]], np.uint8),
            envpos=dict(seeds=pos["seeds"], next_serial=pos["next_serial"],
                        run_start=pos["run_start"], step_count=pos["step_count"],
                        log_base=pos["log_base"],
                        log=np.stack(pos["log"]) if pos["log"] else
                        np.zeros((0, self.cfg.envs), np.uint16)),
        )
        if self.behaviour is not None:
            state["behaviour"] = self.behaviour.state_dict()
        if self.cfg.adv_norm == "per_type":
            state["adv_norm"] = self.advnorm.to_dict()
        tmp = path.with_suffix(".tmp")
        torch.save(state, tmp)
        tmp.replace(path)

    def load(self, path):
        s = torch.load(path, map_location="cpu", weights_only=False)
        self.load_weights(s)
        self._load_rest(s)

    def load_weights(self, s):
        """A checkpoint's (loaded) net, optimiser and EMAs, upgraded from v1 when this learner
        has `obs_v2` and the checkpoint does not; `load` restores the rest."""
        has = lambda prefix: any(k.startswith(prefix) for k in s["net"])  # noqa: E731
        later = self._later_groups() if self.cfg.obs_v2 else []
        # The checkpoint has a prefix of the later groups (v1: none, ABI 8: v2, ABI 9: v2 and
        # side, ABI 10: all).
        kept = 0
        while kept < len(later) and has(later[kept][0]):
            kept += 1
        upgrade = kept < len(later)
        if upgrade:
            # RLN16: a v1 checkpoint into the v2 net; RLN21: into the net with the side inputs;
            # RLN22: into the net with the plan column. The new weights keep their init (zero
            # where it reaches the outputs), Adam's state starts for them alone (a group each),
            # the EMAs start at them, and the probe's rows get a zero tail (no extra inputs).
            self.net.load_v1(s["net"])
            self.opt = self._optimizer(v2_group=False)
            for _, group, _ in later[:kept]:
                self.opt.add_param_group(group())
            self.opt.load_state_dict(s["opt"])
            added = []
            for _, group, params in later[kept:]:
                self.opt.add_param_group(group())
                added += params()
            new = [p.detach().cpu().clone() for p in added]
            s["emas"] = [list(e) + [t.clone() for t in new] for e in s["emas"]]
            if s.get("behaviour") is not None:
                b = E2ENet(d=self.cfg.d, layers=self.cfg.layers, obs_v2=True)
                b.load_v1(s["behaviour"])
                fresh = {k for k in b.state_dict() if k not in s["behaviour"]}
                b.load_state_dict({**b.state_dict(), **{k: v for k, v in self.net.state_dict().items()
                                                        if k in fresh}})
                s["behaviour"] = b.state_dict()
            if s["probe"] is not None and s["probe"]["obs"].shape[1] < self.obs_width:
                o = s["probe"]["obs"]
                pad = torch.zeros(o.shape[0], self.obs_width - o.shape[1], dtype=o.dtype)
                s["probe"]["obs"] = torch.cat([o, pad], 1)
        else:
            self.net.load_state_dict(s["net"])
            self.opt.load_state_dict(s["opt"])
        for g in self.opt.param_groups:  # a checkpoint's own optimiser flags do not carry over
            g["fused"] = True if self.cfg.fused_adam else None
            g["foreach"] = None
        if self.cfg.fused_adam:  # fused AdamW keeps its step counts on the device
            for p, st in self.opt.state.items():
                if "step" in st:
                    st["step"] = st["step"].to(p.device, torch.float32)
        self.emas = [[t.to(self.dev) for t in e] for e in s["emas"]]

    def _load_rest(self, s):
        ad = s.get("adaptive")  # RLN37: absent in an older checkpoint (L = 0: the fixed mix)
        self.adapt_loss = list(ad["loss"]) if ad else [0.0] * 6
        self.adapt_deaths = list(ad["deaths"]) if ad else [0] * 6
        torch.set_rng_state(s["rng"]["torch"])
        self.gen.set_state(s["rng"]["gen"])
        if s["rng"]["cuda"] is not None and torch.cuda.is_available():
            torch.cuda.set_rng_state_all(s["rng"]["cuda"])
        self.iteration, self.decisions = s["iteration"], s["decisions"]
        if self.cfg.overlap:
            # An older checkpoint (no overlap) resumes with its own weights as the behaviour.
            if s.get("behaviour") is not None:
                self.behaviour.load_state_dict(s["behaviour"])
            else:
                self._copy_behaviour()
            if s["rng"].get("cgen") is not None:
                self.cgen.set_state(s["rng"]["cgen"])
            else:
                self.cgen.manual_seed(self.cfg.seed + 2 + self.iteration)
        lo, hi = self.cfg.lr_factor_range  # RLN34: the range may change at a resume
        self.lr_factor = min(hi, max(lo, s["lr_factor"]))
        eb = s.get("ent_boost")  # RLN35; absent in older checkpoints
        if eb is not None:
            self.ent_mult, self.ent_streak = list(eb["mult"]), list(eb["streak"])
            self.ent_boosts = list(eb["boosts"])
        if s.get("adv_norm") is not None:
            self.advnorm = R.AdvNorm.from_dict(s["adv_norm"])
        self.probe = None if s["probe"] is None else {k: v.to(self.dev)
                                                       for k, v in s["probe"].items()}
        self.floors, self.wins = list(s["floors"].tolist()), list(s["wins"].tolist())
        p = s["envpos"]
        self.env_seeds, self.next_serial = p["seeds"].copy(), int(p["next_serial"])
        self.run_start, self.step_count = p["run_start"].copy(), int(p["step_count"])
        self.log_base = int(p["log_base"])
        self.action_log = list(p["log"])
        # A practice episode is not replayed (its snapshot is gone with the pool): the env
        # starts the source's next run instead, after the replay.
        for i in np.nonzero(self.env_seeds == PRACTICE_SEED)[0]:
            self.env_seeds[i] = self.cfg.seed_range[0] + self.next_serial
            self.next_serial += 1
            self.run_start[i] = self.log_base + len(self.action_log)
        # Each env back on its run: restart on its seed, replay its actions.
        self.envs.restart(self.env_seeds, self.next_serial)
        for k, actions in enumerate(self.action_log):
            step = self.log_base + k
            replay = np.where(self.run_start <= step, actions, HOLD).astype(np.uint16)
            self.envs.step(replay)
            if self.envs.done.any():
                raise RuntimeError("a replayed run ended: the env is not where it was")

    # ---- collection ----

    def _h2d(self, x):
        """A host array on the device: `pinned_copies` through pinned memory, non_blocking (the
        caching host allocator keeps the buffer until the copy is done), else as before."""
        if self.cfg.pinned_copies and self.dev.type == "cuda":
            t = torch.empty(x.shape, dtype=torch.from_numpy(x[:0]).dtype, pin_memory=True)
            t.numpy()[...] = x
            return t.to(self.dev, non_blocking=True)
        return torch.from_numpy(x).to(self.dev)

    def _batch_in(self):
        envs = self.envs
        if self.cfg.pinned_copies:
            return (self._h2d(envs.obs[:, :self.obs_width]), self._h2d(envs.masks.view(np.int64)),
                    self._h2d(envs.priv))
        obs = torch.from_numpy(envs.obs[:, :self.obs_width].copy()).to(self.dev, non_blocking=True)
        masks = torch.from_numpy(envs.masks.view(np.int64).copy()).to(self.dev)
        priv = torch.from_numpy(envs.priv.copy()).to(self.dev)
        return obs, masks, priv

    def _autocast(self):
        return torch.autocast(self.dev.type, dtype=torch.bfloat16,
                              enabled=self.dev.type == "cuda")

    def _num_dtype(self):
        return torch.bfloat16 if self.dev.type == "cuda" else torch.float32

    def _forward(self, obs, masks, priv, value=True, net=None, phase=None, masks_host=None):
        """`phase`: the rows' phase bytes on the host, if known (v2: the MAPNODE rows without a
        device sync). `masks_host`: the rows' mask words on the host (`static_head`)."""
        net = net or self.net
        tok = self._encode(obs, masks)
        if self.cfg.obs_v2 and phase is not None:
            tok["y_rows"] = E2ENet.y_rows_of(phase == E.PHASE_MAP, self.dev)
        nleg = (E.legal_counts(masks_host, self.cfg.discard_potion)
                if self.cfg.static_head and masks_host is not None else None)
        ptok = E.encode_priv(priv) if value else None
        with self._autocast():
            if self.cfg.obs_full and self.cfg.collect_buckets > 1:
                logits, v = self._bucketed(net, tok, ptok, value, nleg)
            else:
                if nleg is not None:
                    tok["n_legal"] = int(nleg.sum())
                logits, v = self._unbucketed(net, tok, ptok, value)
        return tok, ptok, logits, v

    @staticmethod
    def _unbucketed(net, tok, ptok, value):
        if value:
            return net(tok, ptok)
        return net.policy(tok)[0], None

    _SHARED = ("akind", "asub")
    """`encode`'s tensors that are not per row."""

    def _bucketed(self, net, tok, ptok, value, nleg=None):
        """RLN31 `collect_buckets`: the net on the rows sorted by token width, in that many
        groups, each cut to its own longest row (a batch's widest rows no longer pad the rest);
        the logits and values back in row order."""
        b = tok["valid"].shape[0]
        host = torch.stack([tok["width"], (tok["phase"] == E.PHASE_MAP).long()]).cpu().numpy()
        width, is_map = host[0], host[1].astype(bool)
        order = np.argsort(width, kind="stable")
        logits = v = None
        for part in np.array_split(order, min(self.cfg.collect_buckets, b)):
            idx = torch.from_numpy(part).to(self.dev)
            sub = {k: x if k in self._SHARED or not torch.is_tensor(x) else x[idx]
                   for k, x in tok.items() if k != "y_rows"}
            sub["t"] = int(width[part].max())
            if nleg is not None:
                sub["n_legal"] = int(nleg[part].sum())
            if self.cfg.obs_v2:
                sub["y_rows"] = E2ENet.y_rows_of(is_map[part], self.dev)
            if value:
                lg, vv = net(sub, {k: x[idx] for k, x in ptok.items()})
            else:
                lg, vv = net.policy(sub)[0], None
            if logits is None:
                logits = lg.new_empty((b, *lg.shape[1:]))
                v = None if vv is None else vv.new_empty((b, *vv.shape[1:]))
            logits[idx] = lg
            if vv is not None:
                v[idx] = vv
        return logits, v

    def _encode(self, obs, masks):
        cfg = self.cfg
        kw = dict(v2=cfg.obs_v2, full=cfg.obs_full, events=cfg.obs_events,
                  discard=cfg.discard_potion)
        if cfg.compile_encode and obs.shape[0] == cfg.envs and self.dev.type == "cuda":
            if self._cencode is None:  # RLN44: fixed shapes, the collect's batch size only
                E.enable_compile()  # the v2 extras' own compile is traced inline, once
                self._cencode = torch.compile(E.encode, dynamic=False)
            return self._cencode(obs, masks, **kw)
        return E.encode(obs, masks, **kw)

    def _alloc(self):
        """An empty batch on the device (allocated by the caller's stream)."""
        cfg, n, s, dev = self.cfg, self.cfg.envs, self.cfg.steps, self.dev
        buf = dict(
            masks=torch.empty(s, n, 6, dtype=torch.int64, device=dev),
            action=torch.empty(s, n, dtype=torch.int64, device=dev),
            logp=torch.empty(s, n, device=dev),
            value=torch.empty(s, n, device=dev),
            reward=torch.empty(s, n, device=dev),
            done=torch.empty(s, n, dtype=torch.bool, device=dev),
            combat=torch.empty(s, n, dtype=torch.bool, device=dev),
            width=torch.empty(s, n, dtype=torch.int16, device=dev),
            last_value=torch.empty(n, device=dev),
        )
        if cfg.practice.enabled:
            buf["boot"] = torch.zeros(s, n, device=dev)
            if cfg.practice.gym() is not None:  # RLN43: the gym's rows (loss weight, stats)
                buf["gym"] = torch.zeros(s, n, dtype=torch.bool, device=dev)
            if cfg.returns == "two_chain":
                buf["trunc"] = torch.zeros(s, n, dtype=torch.bool, device=dev)
        if cfg.kind_log or cfg.explore_floor is not None:  # RLN43: rest rows (Smith), HP band
            buf["rest"] = torch.full((s, n), -2, dtype=torch.int8, device=dev)
            buf["rest_hp"] = torch.zeros(s, n, dtype=torch.uint8, device=dev)
        if self.prior is not None:  # RLN42: each row's prior entries (ids into `PriorKL`)
            buf["pent"] = torch.full((s, n, 3), -1, dtype=torch.int64, device=dev)
        if cfg.returns == "two_chain" or cfg.adv_norm == "per_type":
            buf["dtype"] = torch.empty(s, n, dtype=torch.uint8, device=dev)
        if cfg.cache_tokens:
            buf["packed"] = {k: torch.empty(s, n, *shape, dtype=dt, device=dev)
                             for k, (shape, dt)
                             in E.packed_shapes(self._num_dtype(), self.cfg.obs_v2,
                                                 self.cfg.obs_full,
                                                 self.cfg.obs_events).items()}
            if self.probe is None:  # the probe's rows are the first batch's first rows
                k = min(-(-cfg.probe_rows // n), s)
                buf["probe_obs"] = torch.empty(k, n, self.obs_width, dtype=torch.uint8,
                                               device=dev)
        else:
            buf["obs"] = torch.empty(s, n, self.obs_width, dtype=torch.uint8, device=dev)
            buf["priv"] = torch.empty(s, n, E.PRIV_BYTES, dtype=torch.uint8, device=dev)
        return buf

    @torch.no_grad()
    def _collect_into(self, buf, net, gen):
        """Steps the envs `steps` times with `net`, sampling with `gen`, into `buf`."""
        cfg, s = self.cfg, self.cfg.steps
        envs = self.envs
        rw = cfg.reward
        runs = dict(floors=[], wins=0, ends={}, tokens=0.0, overflow=0, empty={}, discards=0, offers=0)
        adaptive = cfg.practice.enabled and cfg.practice.adaptive
        if adaptive:
            runs["deaths"] = {}
        practice = {} if "boot" in buf else None
        pending = []  # RLN26: the truncated practice episodes' final rows, bootstrapped at the end
        tokens = torch.zeros((), dtype=torch.float64, device=self.dev)
        overflow = torch.zeros((), dtype=torch.int64, device=self.dev)
        packed = buf.get("packed")
        most = [] if cfg.static_head else None  # RLN44: `pack`'s check, once at the end
        # RLN25 (`STS2_COLLECT_TIMING=1`): where the collect's wall time goes. Host seconds per
        # phase (perf_counter, no added syncs) and the GPU's own forward and sampling time (CUDA
        # events, read once at the end): `wait` is `.cpu()` blocking on the GPU, `step` the Rust
        # env step, `practice` (inside `rest`) the bootstrap forwards of ended practice episodes.
        timed = COLLECT_TIMING
        clock = time.perf_counter if timed else (lambda: 0.0)
        host = dict(batch_in=0.0, forward=0.0, sample=0.0, wait=0.0, step=0.0, rest=0.0,
                    practice=0.0)
        cuda = timed and self.dev.type == "cuda"
        events = []
        prior, beh = self.prior, self.telemetry
        if prior is not None or beh is not None:
            import rln42_prior
            scans, pstats, gold = [], {}, []
        for t in range(s):
            c0 = clock()
            obs, masks, priv = self._batch_in()
            c1 = clock()
            if cuda:
                e0 = torch.cuda.Event(enable_timing=True)
                e0.record()
            tok, ptok, logits, v = self._forward(obs, masks, priv, net=net,
                                                 phase=envs.obs[:, 3], masks_host=envs.masks)
            if cuda:
                e1 = torch.cuda.Event(enable_timing=True)
                e1.record()
            c2 = clock()
            # Gumbel-max: argmax(logits - log(Exp(1))) samples the softmax.
            u = torch.rand(logits.shape, generator=gen, device=self.dev)
            expo = (-torch.log(u.clamp(min=1e-30))).clamp(min=1e-30)
            a = (logits - torch.log(expo)).argmax(-1)
            logp = torch.log_softmax(logits, -1).gather(1, a.unsqueeze(1)).squeeze(1)
            if cuda:
                e2 = torch.cuda.Event(enable_timing=True)
                e2.record()
                events.append((e0, e1, e2))
            buf["masks"][t] = masks
            if packed is not None:
                for k, x in E.pack(tok, ptok, self._num_dtype(), most).items():
                    packed[k][t] = x
                if "probe_obs" in buf and t < buf["probe_obs"].shape[0]:
                    buf["probe_obs"][t] = obs
            else:
                buf["obs"][t], buf["priv"][t] = obs, priv
            buf["action"][t], buf["logp"][t], buf["value"][t] = a, logp, v.float()
            buf["combat"][t] = tok["combat"]
            if "dtype" in buf:
                buf["dtype"][t] = self._h2d(
                    R.decision_types(envs.obs, envs.masks.view(np.uint8), E.SELECT_AT))
            buf["width"][t] = tok["width"]
            if "rest" in buf:
                rr, rb = rest_rows(envs.obs)
                buf["rest"][t], buf["rest_hp"][t] = self._h2d(rr), self._h2d(rb)
            tokens += tok["count"].double().mean()
            overflow += tok["overflow"].sum()
            if prior is not None or beh is not None:  # host work while the GPU runs the forward
                entry = self.shops(envs.obs)
                sc = rln42_prior.scan(envs.obs, envs.masks, entry=entry)
                enter = np.nonzero(entry)[0]
                gold += (envs.obs[enter, 12].astype(np.int64)
                         | envs.obs[enter, 13].astype(np.int64) << 8).tolist()
            c3 = clock()
            actions = a.cpu().numpy()
            if prior is not None:
                rln42_prior.choice_stats(pstats, sc, actions)
                scans.append((t, sc))
            if beh is not None:
                beh.step(sc, actions, envs.obs[:, 3])
            c4 = clock()
            empty = np.nonzero(~envs.masks.any(1))[0]  # no decision (`Learner._drop_empty`)
            if self.cfg.discard_potion:  # RLN32: indices 355-360 are bits 35-40 of mask word 5
                runs["offers"] += int(((envs.masks[:, 5] >> np.uint64(35)) & np.uint64(63) != 0).sum())
                runs["discards"] += int(((actions >= 355) & (actions < 361)).sum())
            up0 = deck_upgrades(envs.obs) if rw.upgrade else None
            hp_on = practice is not None and cfg.practice.hp_coef > 0
            gym = cfg.practice.gym() if "gym" in buf else None
            if hp_on:  # RLN44: the practice rows (own snapshots) and their HP before the step
                prac0 = (envs.cell != 0) & (envs.cell < GYM_CODE)
                hp0 = hp_fraction(envs.obs)
            if gym is not None:  # RLN43: the gym rows (cell codes 7-15) before the step
                gym0 = envs.cell >= GYM_CODE
                ghp0 = hp_fraction(envs.obs) if gym.hp_coef > 0 else None
                ged0 = enemy_dealt(envs.obs) if gym.enemy_coef > 0 else None
                gpot0 = belt_count(envs.obs) if gym.pot_coef > 0 else None
            envs.step(actions)
            c5 = clock()
            for e in envs.end[empty].tolist():  # how each of those episodes ended
                runs["empty"][RUN_ENDS[e]] = runs["empty"].get(RUN_ENDS[e], 0) + 1
            ev = envs.events.astype(np.float32)
            done = envs.done.astype(bool)
            reward = rw.floor * ev[:, 0] + rw.boss * ev[:, 1] + rw.win * ev[:, 3]
            if up0 is not None:  # RLN40: the upgrade bonus (a done row's deck is its final one)
                after = np.where(done[:, None], envs.final_obs, envs.obs)
                reward = reward + rw.upgrade * (deck_upgrades(after) - up0)
            if hp_on:  # RLN44 `practice.hp_coef`: c * (phi(after) - phi(before)), practice rows
                hp1 = np.where(done, hp_fraction(envs.final_obs), hp_fraction(envs.obs))
                hp1 = np.where(done & (envs.end == RUN_ENDS.index("died")), 0.0, hp1)
                shaped = np.where(prac0, cfg.practice.hp_coef * (hp1 - hp0), 0.0)
                reward = (reward + shaped).astype(np.float32)
                runs["hp_reward"] = runs.get("hp_reward", 0.0) + float(shaped.sum())
            if gym is not None:
                reward = self._gym_reward(reward, gym, gym0, ghp0, done, runs, ged0, gpot0)
                buf["gym"][t] = self._h2d(gym0)
            buf["reward"][t] = self._h2d(reward)
            buf["done"][t] = self._h2d(done)
            if beh is not None and done.any():
                prac = (envs.ended_cell > 0) & done if practice is not None else np.zeros_like(done)
                beh.ended(done & ~prac, prac, envs.final_obs, envs.floor, envs.won,
                          envs.end == RUN_ENDS.index("died"), deck_upgrades(envs.final_obs))
            if practice is not None:
                self.ep_bosses += envs.events[:, 1]
            if practice is not None and done.any():
                ended = envs.ended_cell.copy()
                p0 = clock()
                self._practice_ends(buf, t, done, ended, practice, pending)
                host["practice"] += clock() - p0
                self.ep_bosses[done] = 0
                done = done & (ended == 0)  # the run statistics count normal runs only
            if adaptive and done.any():
                self._tally_deaths(runs["deaths"], done)
            if done.any():
                fl = envs.floor[done].astype(np.int64).tolist()
                wn = envs.won[done].astype(np.int64).tolist()
                runs["floors"] += fl
                runs["wins"] += sum(wn)
                self.floors += fl
                self.wins += wn
                for e in envs.end[done].tolist():
                    runs["ends"][RUN_ENDS[e]] = runs["ends"].get(RUN_ENDS[e], 0) + 1
            self._track(actions, envs.done.astype(bool))
            c6 = clock()
            for k, d in zip(host, (c1 - c0, c2 - c1, c3 - c2, c4 - c3, c5 - c4, c6 - c5)):
                host[k] += d
        if prior is not None:
            ts, rows, slots, ids = prior.commit(scans)
            buf["pent"][torch.from_numpy(ts).to(self.dev), torch.from_numpy(rows).to(self.dev),
                        torch.from_numpy(slots).to(self.dev)] = torch.from_numpy(ids).to(self.dev)
            runs["prior"] = dict(rln42_prior.rates(pstats), entries=int(len(ids)),
                                 shop_gold=round(float(np.mean(gold)), 1) if gold else None,
                                 shops=len(gold))
        E.check_full_slots(most)
        p0 = clock()
        self._practice_bootstrap(buf, pending, net)
        host["practice"] += clock() - p0
        obs, masks, priv = self._batch_in()
        _, _, _, last_v = self._forward(obs, masks, priv, net=net, phase=envs.obs[:, 3],
                                        masks_host=envs.masks)
        buf["last_value"].copy_(last_v.float())
        runs["tokens"] = float(tokens) / s
        runs["overflow"] = int(overflow)
        if timed:
            timing = {k: round(v, 4) for k, v in host.items()}
            if events:
                events[-1][2].synchronize()
                timing["gpu_forward"] = round(
                    sum(a.elapsed_time(b) for a, b, _ in events) / 1e3, 4)
                timing["gpu_sample"] = round(
                    sum(b.elapsed_time(c) for _, b, c in events) / 1e3, 4)
            runs["timing"] = timing
        if practice is not None:
            runs["practice"] = practice
        return runs

    def _gym_reward(self, reward, gym, gym0, ghp0, done, runs, ged0=None, gpot0=None):
        """RLN43: a gym row's reward: none of the run's (floors, bosses, win); its cell's
        `win_bonus` on the row that ends a survived fight; `hp_coef` * (phi after - before)."""
        envs = self.envs
        out = np.where(gym0, 0.0, reward)
        code = envs.ended_cell.astype(np.int64)
        cell = code - GYM_CODE
        inside = (cell >= 0) & (cell < len(GYM_CELL_NAMES))
        survived = done & gym0 & inside & ((envs.end == GYM_END)
                                           | (envs.end == RUN_ENDS.index("victory")))
        bonus = np.where(survived, gym.bonus()[np.clip(cell, 0, len(GYM_CELL_NAMES) - 1)], 0.0)
        out = out + bonus
        g = runs.setdefault("gym", dict(rows=0, bonus=0.0, hp=0.0))
        g["rows"] += int(gym0.sum())
        g["bonus"] += float(bonus.sum())
        if ghp0 is not None:
            hp1 = np.where(done, hp_fraction(envs.final_obs), hp_fraction(envs.obs))
            hp1 = np.where(done & (envs.end == RUN_ENDS.index("died")), 0.0, hp1)
            shaped = np.where(gym0, gym.hp_coef * (hp1 - ghp0), 0.0)
            out = out + shaped
            g["hp"] += float(shaped.sum())
        died = done & (envs.end == RUN_ENDS.index("died"))
        if ged0 is not None:  # CB2 `enemy_coef`: 1 on a win; a death keeps its last combat obs
            fin = envs.final_obs
            fin_combat = fin[:, HEADER_PHASE] == PHASE_COMBAT
            ed1 = np.where(done, np.where(fin_combat, enemy_dealt(fin), ged0), enemy_dealt(envs.obs))
            ed1 = np.where(survived, 1.0, ed1)
            shaped = np.where(gym0, gym.enemy_coef * (ed1 - ged0), 0.0)
            out = out + shaped
            g["enemy"] = g.get("enemy", 0.0) + float(shaped.sum())
            dg = died & gym0
            g["deaths"] = g.get("deaths", 0) + int(dg.sum())
            g["dealt_at_death"] = g.get("dealt_at_death", 0.0) + float(ed1[dg].sum())
        if gpot0 is not None:  # CB2 `pot_coef`: potions kept, 0 after a death
            p1 = np.where(done, belt_count(envs.final_obs), belt_count(envs.obs))
            p1 = np.where(died, 0.0, p1)
            shaped = np.where(gym0, gym.pot_coef * (p1 - gpot0), 0.0)
            out = out + shaped
            g["pot"] = g.get("pot", 0.0) + float(shaped.sum())
        return out.astype(np.float32)

    def _tally_deaths(self, out, done):
        """RLN37: the normal runs that died at this step, per practice cell (`out[cell name]`;
        `out["other"]` a hallway fight or a non-combat room), by the header of their last obs."""
        envs = self.envs
        for i in np.nonzero(done & (envs.end == RUN_ENDS.index("died")))[0]:
            c = death_cell(envs.final_obs[i])
            k = "other" if c is None else CELL_NAMES[c]
            out[k] = out.get(k, 0) + 1

    def _adaptive_update(self, deaths):
        """RLN37: a collect's deaths into D; every `adaptive_every` iterations L = decay * L + D."""
        pr = self.cfg.practice
        for c, name in enumerate(CELL_NAMES):
            self.adapt_deaths[c] += deaths.get(name, 0)
        if self.iteration % pr.adaptive_every == 0:
            self.adapt_loss = pr.adaptive_step(self.adapt_loss, self.adapt_deaths)
            self.adapt_deaths = [0] * 6

    def _practice_ends(self, buf, t, done, ended, out, pending):
        """Practice episodes that ended at step `t`: the truncated ones' final rows are queued
        in `pending` (bootstrapped by `_practice_bootstrap` once the collect ends; the net does
        not change meanwhile) and flagged in `buf["trunc"]`, and per cell [ended, fight survived
        (truncated, won, or a boss cell's boss killed)] goes into `out`, with the end kinds
        (`out["ends"]`) and the boss cells' kills (`out["kills"]`)."""
        envs = self.envs
        trunc = done & (envs.end == PRACTICE_END)
        if trunc.any():
            idx = np.nonzero(trunc)[0]
            # The env overwrites its final buffers at the next end: copy them now.
            pending.append((t, idx, envs.final_obs[idx, :self.obs_width].copy(),
                            envs.final_masks[idx].view(np.int64).copy(),
                            envs.final_priv[idx].copy()))
            if "trunc" in buf:
                buf["trunc"][t, torch.from_numpy(idx).to(self.dev)] = True
        practice_ends(out, done, ended, envs.end, envs.won, self.ep_bosses)

    def _practice_bootstrap(self, buf, pending, net, chunk=PRACTICE_CHUNK):
        """The value of every queued final row into `buf["boot"][t, env]`: batched forwards of at
        most `chunk` rows (RLN26; it was one forward per step with a truncation)."""
        if not pending:
            return
        ts = np.concatenate([np.full(len(idx), t, np.int64) for t, idx, *_ in pending])
        es = np.concatenate([idx for _, idx, *_ in pending])
        obs, masks, priv = (np.concatenate([p[k] for p in pending]) for k in (2, 3, 4))
        ts, es = (torch.from_numpy(x).to(self.dev) for x in (ts, es))
        for a in range(0, len(es), chunk):
            b = slice(a, a + chunk)
            _, _, _, v = self._forward(*(torch.from_numpy(x[b]).to(self.dev)
                                         for x in (obs, masks, priv)), net=net,
                                       masks_host=masks[b],
                                       phase=obs[b, 3] if self.cfg.static_head else None)
            buf["boot"][ts[b], es[b]] = v.float()

    def _finish(self, buf):
        """GAE (on the caller's stream)."""
        cfg = self.cfg
        if cfg.returns == "two_chain":
            return self._finish_two_chain(buf)
        if cfg.returns != "chained":
            raise ValueError(f"returns {cfg.returns!r}: want 'chained' or 'two_chain'")
        adv, ret = gae(buf["reward"], buf["value"], buf["done"], buf["combat"],
                       buf.pop("last_value"), cfg.lam_combat, cfg.lam_run, buf.pop("boot", None))
        buf["adv"], buf["ret"] = adv, ret
        return buf

    _INPUTS = ("masks", "action", "logp", "value", "combat", "width", "dtype", "obs", "priv",
               "pent", "gym", "rest", "rest_hp")
    _HELD_FILL = dict(gym=False, rest=-2, rest_hp=0)  # inputs turned on at a resume

    def _finish_two_chain(self, buf):
        """Two-chain returns over the held rows and this batch (`sts2.e2e.returns`). The batch's
        rows are flattened step-major (row s·n + e); `buf["flat"]` gets the rows that train
        now, with their advantages and targets, and the held rows' inputs stay for the next
        iteration."""
        cfg, s, n = self.cfg, self.cfg.steps, self.cfg.envs
        fresh_in = {k: buf[k].flatten(0, 1) for k in self._INPUTS if k in buf}
        if "packed" in buf:
            fresh_in["packed"] = {k: v.flatten(0, 1) for k, v in buf["packed"].items()}
        if self.probe is None:  # the probe's rows are the first batch's first rows, as `update`
            k = min(cfg.probe_rows, s * n)
            obs = (buf["probe_obs"] if "probe_obs" in buf else buf["obs"]).flatten(0, 1)
            self.probe = dict(obs=obs[:k].clone(), masks=fresh_in["masks"][:k].clone())
        cpu = lambda k: buf[k].cpu().numpy()  # noqa: E731
        reward, done, combat = cpu("reward").astype(np.float64), cpu("done"), cpu("combat")
        value, dtype = cpu("value").astype(np.float64), cpu("dtype")
        last_v = buf.pop("last_value").cpu().numpy().astype(np.float64)
        boot = buf.pop("boot").cpu().numpy().astype(np.float64) if "boot" in buf else None
        trunc = buf.pop("trunc").cpu().numpy() if "trunc" in buf else np.zeros_like(done)
        # Run uids per row: an env's run changes after each done row.
        uid = np.empty((s, n), np.int64)
        for t in range(s):
            uid[t] = self.run_uid
            ended = np.nonzero(done[t])[0]
            self.run_uid[ended] = self.next_uid + np.arange(ended.size)
            self.next_uid += ended.size
        tt = 2 * (self.row_t + np.arange(s, dtype=np.int64))[:, None] + np.zeros((1, n), np.int64)
        self.row_t += s
        rows = dict(run_uid=uid.ravel(), t=tt.ravel(), is_combat=combat.ravel(),
                    dtype=dtype.ravel().astype(np.int64), reward=reward.ravel(),
                    done=(done & ~trunc).ravel(), bootstrap=np.zeros(s * n, bool),
                    value=value.ravel(), ref=np.arange(s * n, dtype=np.int64))
        # Bootstrap rows: a truncated practice segment's (done set, its boot value), and the
        # cut of every run still open at the end of the batch (the next state's value).
        ts, es = np.nonzero(trunc)
        open_e = np.nonzero(~done[-1])[0]
        nb = ts.size + open_e.size
        bt = dict(run_uid=np.concatenate([uid[ts, es], self.run_uid[open_e]]),
                  t=np.concatenate([tt[ts, es] + 1, np.full(open_e.size, 2 * self.row_t)]),
                  is_combat=np.zeros(nb, bool), dtype=np.zeros(nb, np.int64),
                  reward=np.zeros(nb), done=np.concatenate([np.ones(ts.size, bool),
                                                            np.zeros(open_e.size, bool)]),
                  bootstrap=np.ones(nb, bool),
                  value=np.concatenate([boot[ts, es] if boot is not None else np.zeros(0),
                                        last_v[open_e]]),
                  ref=np.full(nb, -1, np.int64))
        fresh = {k: np.concatenate([rows[k], bt[k]]) for k in rows}
        n_held = len(self.held["run_uid"]) if self.held else 0
        if n_held:
            fresh["ref"] = np.where(fresh["ref"] >= 0, fresh["ref"] + n_held, -1)
        table = R.join(self.held, fresh)
        hold = cfg.hold_iters
        ret = R.compute_returns(table, lam_c=cfg.lam_combat, lam_r=cfg.lam_run, hold_iters=hold)
        while hold > 0 and int(ret["held"].sum()) > cfg.held_max:  # RLN42 `held_max`
            hold -= 1
            ret = R.compute_returns(table, lam_c=cfg.lam_combat, lam_r=cfg.lam_run,
                                    hold_iters=hold)

        def gather(refs, host=False):
            """The inputs of rows `refs` (held ones first, then fresh, each in `refs` order)
            and that order as a permutation of `refs`; `host`: on the CPU. The held rows'
            inputs live on the CPU (NECRO4: 330k held rows outgrew the GPU)."""
            perm = np.concatenate([np.flatnonzero(refs < n_held), np.flatnonzero(refs >= n_held)])
            nh = int((refs < n_held).sum())
            hi = torch.from_numpy(refs[perm][:nh])
            fi = torch.from_numpy(refs[perm][nh:] - n_held).to(self.dev)
            held_in = self.held_in or {}

            def pick(h, f, k=None):
                x = f[fi]
                if host:
                    x = x.cpu()
                if h is None and nh and k in self._HELD_FILL:  # held before the input existed
                    h = torch.full((n_held, *x.shape[1:]), self._HELD_FILL[k], dtype=x.dtype)
                if h is None:
                    return x
                return torch.cat([h[hi] if host else h[hi].to(self.dev), x])
            out = {k: pick(held_in.get(k), v, k) for k, v in fresh_in.items() if k != "packed"}
            if "packed" in fresh_in:
                hp = held_in.get("packed", {})
                out["packed"] = {k: pick(hp.get(k), v) for k, v in fresh_in["packed"].items()}
            return out, perm

        tr = np.flatnonzero(ret["trained"])
        flat, perm = gather(table["ref"][tr])
        rows_tr = tr[perm]
        flat["adv"] = torch.from_numpy(ret["adv"][rows_tr]).float().to(self.dev)
        flat["ret"] = torch.from_numpy(ret["target"][rows_tr]).float().to(self.dev)
        # Held rows, in ref order (held part first), renumbered 0.. as their inputs are kept.
        new_held = R.carry(table, ret)
        if new_held:
            srt = np.argsort(new_held["ref"], kind="stable")
            new_held = {k: v[srt] for k, v in new_held.items()}
            self.held_in, _ = gather(new_held["ref"], host=True)
            new_held["ref"] = np.arange(len(srt), dtype=np.int64)
        else:
            self.held_in = None
        self.held = new_held
        comb = table["is_combat"].astype(bool)
        trn = ret["trained"]
        bw = ret["boot_w"]
        buf["flat"] = flat
        buf["ret_stats"] = dict(
            rows_trained=int(trn.sum()), rows_trained_run=int((trn & ~comb).sum()),
            rows_held=len(self.held["run_uid"]) if self.held else 0,
            rows_overdue=int(ret["overdue"].sum()), hold_iters=hold,
            rows_practice_cut=int(ts.size),
            boot_w_combat=float(np.mean(bw[trn & comb])) if (trn & comb).any() else None,
            boot_w_run=float(np.mean(bw[trn & ~comb])) if (trn & ~comb).any() else None)
        return buf

    def collect(self):
        """A synchronous batch with the current weights (`overlap` off)."""
        self._practice_apply()
        buf = self._alloc()
        runs = self._collect_into(buf, self.net, self.gen)
        return self._finish(buf), runs

    def _order_collect(self):
        """The collect stream starts after the work queued so far (the batch's allocation,
        the behaviour copy) and not after the update queued next; called on the main thread."""
        if self.cstream is not None:
            self.cstream.wait_stream(torch.cuda.current_stream(self.dev))

    def _collect_ahead(self, result):
        """The thread body: the next batch with `behaviour`, on `cstream`."""
        try:
            t0 = time.perf_counter()
            buf = result["buf"]
            if self.cstream is not None:
                with torch.cuda.stream(self.cstream):
                    runs = self._collect_into(buf, self.behaviour, self.cgen)
                self.cstream.synchronize()
            else:
                runs = self._collect_into(buf, self.behaviour, self.cgen)
            result["runs"] = runs
            result["secs"] = time.perf_counter() - t0
        except BaseException as e:  # noqa: BLE001 - re-raised on the main thread
            result["error"] = e

    # ---- update ----

    def _probe_logp(self):
        obs, masks = self.probe["obs"], self.probe["masks"]
        step = self.cfg.probe_chunk or obs.shape[0]
        out = []
        for i in range(0, obs.shape[0], step):
            tok = self._encode(obs[i:i + step], masks[i:i + step])
            with torch.no_grad(), self._autocast():
                logits, _, _ = self.net.policy(tok)
            out.append(torch.log_softmax(logits.float(), -1))
        return torch.cat(out)

    def _batch_probe(self, flat):
        """RLN14: this iteration's KL probe, redrawn from the batch as spire-mentor's is
        (`probe_strata`): the rows sorted by token width, cut into strata of ~`probe_rows`/24
        rows, 24 of them drawn. Own numpy RNG, so the training streams are untouched."""
        cfg = self.cfg
        widths = flat["width"].cpu().numpy()
        n_strata = min(widths.shape[0], max(1, round(widths.shape[0] * 24 / cfg.probe_rows)))
        strata = np.array_split(np.argsort(widths, kind="stable"), n_strata)
        rng = np.random.default_rng([cfg.seed, self.iteration, 1])
        pick = np.sort(rng.choice(n_strata, size=min(24, n_strata), replace=False))
        return np.concatenate([strata[j] for j in pick]), widths

    def _batch_logp(self, flat, rows, widths, may_y=None):
        """`may_y` (RLN44, `static_head`): the rows' host MAPNODE flags (`update`), so the trunk
        does not sync on them."""
        step = self.cfg.probe_chunk or len(rows)
        out = []
        for i in range(0, len(rows), step):
            part = rows[i:i + step]
            tok, _ = self._micro_in(flat, torch.from_numpy(part).to(self.dev),
                                    int(widths[part].max()), part)
            if may_y is not None:
                tok["y_rows"] = E2ENet.y_rows_of(may_y[part], self.dev)
            with torch.no_grad(), self._autocast():
                logits, _, _ = self.net.policy(tok)
            out.append(torch.log_softmax(logits.float(), -1))
        return torch.cat(out)

    def _kl(self, old, new=None):
        new = self._probe_logp() if new is None else new
        p = old.exp()
        return float(torch.where(torch.isfinite(old), p * (old - new),
                                 torch.zeros_like(p)).sum(-1).mean())

    def _micro_in(self, flat, idx, t, rows_h=None):
        """The token dicts of rows `idx`, cut to `t` tokens (their longest row); `rows_h`: the
        same rows on the host, for `static_head`'s pair count."""
        if "packed" in flat:
            packed = {k: v[idx] for k, v in flat["packed"].items()}
            tok, ptok = E.unpack(packed, flat["masks"][idx], t, self.cfg.discard_potion)
        else:
            tok = self._encode(flat["obs"][idx], flat["masks"][idx])
            tok["t"] = t
            ptok = E.encode_priv(flat["priv"][idx])
        if self._nleg is not None and rows_h is not None:
            tok["n_legal"] = int(self._nleg[rows_h].sum())
        return tok, ptok

    def _try_compile(self, flat):
        """Compile the blocks and prove them with a dry forward/backward; else stay eager."""
        try:
            self.net.enable_compile()
            if self.cfg.compile_net:
                if not self.cfg.static_head:
                    raise ValueError("compile_net needs static_head (no data-dependent shapes)")
                # Bucketed widths and pair counts: every shape seen gets its graph.
                dynamo.config.cache_size_limit = max(64, dynamo.config.cache_size_limit)
                # RLN45: no duck shaping. Two inputs' sizes that happen to be equal would share
                # one symbol, and inductor then failed to codegen a view of their product
                # ("CantSplit: 256*s3 not divisible by ((s3**2)//s3)", the gym's batches).
                fx_config.use_duck_shape = False
                self.net.forward = _guarded(torch.compile(self.net.forward, dynamic=True),
                                            self.net.forward, "forward")
                self.net.policy = _guarded(torch.compile(self.net.policy, dynamic=True),
                                           self.net.policy, "policy")
            if self.cfg.obs_v2:
                E.enable_compile()  # RLN17: the encoder's v2 maths (falls back to eager itself)
            idx = torch.arange(min(64, flat["action"].shape[0]), device=self.dev)
            t = int(flat["width"][idx].max())
            tok, ptok = self._micro_in(flat, idx, t)
            if self.cfg.static_head:
                tok["n_legal"] = int(E.legal_counts(flat["masks"][idx].cpu().numpy(),
                                                    self.cfg.discard_potion).sum())
            with self._autocast():
                logits, v = self.net(tok, ptok)
            (masked_stats(logits.float())[0].nan_to_num(neginf=0).sum() + v.float().sum()).backward()
            self.net.zero_grad(set_to_none=True)
            self.compiled = True
            print("torch.compile: on", flush=True)
        except Exception as e:  # noqa: BLE001 - any failure of the compiled build means eager
            print(f"torch.compile unavailable, running eager: {type(e).__name__}: "
                  f"{str(e)[:300]}", flush=True)
            for blk in (*self.net.blocks, self.net.v_block):
                blk.__dict__.pop("forward", None)
                blk.__dict__.pop("forward2", None)
                blk.__dict__.pop("forward_first", None)
            for k in ("_v2_trunk", "forward", "policy"):
                self.net.__dict__.pop(k, None)
            E.disable_compile()
            self.net.zero_grad(set_to_none=True)
        self.cfg.compile = self.compiled

    @staticmethod
    def _drop_empty(flat, rows):
        """RLN18: the rows with an empty mask are no decisions: an episode that ended at its
        reset (a practice start whose fight is over before its first decision) leaves a zero
        observation and mask for one step, the env takes no action and reports it done on the
        next step. Their logits are all -inf and their log-probs NaN; they leave the update.
        Each is a one-row episode (done before and on it), so no other row's return uses it."""
        live = flat["masks"].ne(0).any(1)
        empty = rows - int(live.sum())
        if not empty:
            return flat, rows, 0
        keep = live.nonzero().squeeze(1)

        def take(k, v):
            return v[keep] if k != "probe_obs" and torch.is_tensor(v) and v.shape[0] == rows else v
        out = {k: take(k, v) for k, v in flat.items() if k != "packed"}
        if "packed" in flat:
            out["packed"] = {k: v[keep] for k, v in flat["packed"].items()}
        return out, rows - empty, empty

    def _micros(self, widths):
        """A minibatch's micros as (start, end): `micro` rows each, or with `obs_full` and
        `micro_width` at most `micro * micro_width` tokens (rows times their longest row, in
        the minibatch's order: sorted by width with `sort_micro`)."""
        cfg, n = self.cfg, len(widths)
        if not (cfg.obs_full and cfg.micro_width):
            return [(a, min(a + cfg.micro, n)) for a in range(0, n, cfg.micro)]
        budget = cfg.micro * cfg.micro_width
        out, a, most = [], 0, 0
        for i, w in enumerate(widths.tolist()):
            most_i = max(most, w)
            if i > a and (i + 1 - a) * most_i > budget:
                out.append((a, i))
                a, most_i = i, w
            most = most_i
        out.append((a, n))
        return out

    def update(self, buf):
        cfg = self.cfg
        if "flat" in buf:  # two-chain returns: the rows that train now (`_finish_two_chain`)
            flat = buf["flat"]
        else:
            flat = {k: v.flatten(0, 1) for k, v in buf.items() if torch.is_tensor(v)}
            if "packed" in buf:
                flat["packed"] = {k: v.flatten(0, 1) for k, v in buf["packed"].items()}
        if cfg.compile and not self.compiled and not getattr(self, "_compile_tried", False):
            self._compile_tried = True
            self._try_compile(flat)
        rows = flat["action"].shape[0]
        if self.probe is None:
            k = min(cfg.probe_rows, rows)
            obs = flat["probe_obs"] if "probe_obs" in flat else flat["obs"]
            self.probe = dict(obs=obs[:k].clone(), masks=flat["masks"][:k].clone())
        flat, rows, empty = self._drop_empty(flat, rows)
        widths = flat["width"].cpu().numpy()
        self._nleg = (E.legal_counts(flat["masks"].cpu().numpy(), cfg.discard_potion)
                      if cfg.static_head else None)
        # v2: per row whether it may have MAPNODE tokens, on the host (`E2ENet.y_rows_of`).
        may_y = None
        if cfg.obs_v2:
            may_y = (flat["packed"]["y_valid"].any(1) if "packed" in flat
                     else flat["obs"][:, 3] == E.PHASE_MAP).cpu().numpy()
        adv = flat["adv"].clone()
        combat = flat["combat"]
        if cfg.adv_norm == "per_type":  # spire-mentor's per-type moments (`sts2.e2e.returns`)
            a = self.advnorm.update_normalize(adv.double().cpu().numpy(),
                                              flat["dtype"].cpu().numpy(),
                                              combat.cpu().numpy())
            adv = torch.from_numpy(a).float().to(self.dev)
        elif cfg.adv_norm == "global":
            for m in (combat, ~combat):
                if int(m.sum()) > 1:
                    a = adv[m]
                    adv[m] = (a - a.mean()) / (a.std() + 1e-8)
        else:
            raise ValueError(f"adv_norm {cfg.adv_norm!r}: want 'global' or 'per_type'")
        warm = min(1.0, (self.iteration + 1) / cfg.warmup_iters)
        # RLN29 `v_warm`: the first updates move the value module only.
        v_only = self.iteration < cfg.v_warm
        value_group = self.opt.param_groups[1]
        for g in self.opt.param_groups:
            g["lr"] = (0.0 if v_only and g is not value_group
                       else g["base"] * self.lr_factor * warm)
        ent_c, ent_r = entropy_coef(cfg, self.decisions)
        if cfg.ent_boost:
            ent_c, ent_r = ent_c * self.ent_mult[0], ent_r * self.ent_mult[1]
        epochs = cfg.epochs if self.decisions < cfg.e1_at else 1
        gym_cfg = cfg.practice.gym()
        gym_w = None
        mass = None
        if gym_cfg is not None and "gym" in flat:
            g = flat["gym"].float()
            if gym_cfg.loss_weight != 1.0:
                gym_w = float(gym_cfg.loss_weight)
            wts = torch.where(flat["gym"], gym_cfg.loss_weight, 1.0)
            run_rows = (~combat).float()
            mass = dict(rows_gym=float(g.mean()), rows_run=float(run_rows.mean()),
                        mass_run=float((wts * run_rows).sum() / wts.sum()),
                        mass_gym=float((wts * g).sum() / wts.sum()))
        kt = self.kinds
        ex = self.explore
        ex_c = ex.coef_at(self.decisions) if ex is not None and not v_only else 0.0
        ex_on = sorted(ex.active(self.kind_ent)) if ex is not None and ex_c > 0 else []
        if kt is not None:  # RLN43: per kind [entropy sum, rows, KL sum]; named P; Smith bands
            k_acc = {k: torch.zeros(3, dtype=torch.float64, device=self.dev) for k in EXPLORE_KINDS}
            k_named = {k: {nm: torch.zeros(2, dtype=torch.float64, device=self.dev)
                           for nm in kt.named[k]} for k in EXPLORE_KINDS}
            smith = torch.zeros(len(REST_BANDS) + 1, 2, dtype=torch.float64, device=self.dev)
        use_prior = self.prior is not None and not v_only
        if use_prior:
            import rln42_prior
            beta = self.prior.cfg.beta(self.decisions)
            pkl = rln42_prior.KindStats(self.dev)
        if not cfg.fixed_probe and cfg.kl_source != "batch":
            raise ValueError("fixed_probe=False needs kl_source='batch' (the fixed probe drives it)")
        old_probe = self._probe_logp() if cfg.fixed_probe else None
        brows, bwidths = self._batch_probe(flat)
        y_probe = may_y if cfg.static_head else None
        old_batch = self._batch_logp(flat, brows, bwidths, y_probe)
        kls_b = []
        acc = torch.zeros(3, dtype=torch.float64, device=self.dev)  # pg v clip
        ent_acc = FamilyEntropy(self.dev)
        n_mb, kls, epochs_done = 0, [], 0
        for _ in range(epochs):
            perm = torch.randperm(rows, generator=self.gen, device=self.dev)
            perm_h = perm.cpu().numpy()
            if cfg.sort_micro:  # the same minibatches, each split into micros by token width
                for mb in range(0, rows, cfg.minibatch):
                    part = perm_h[mb:mb + cfg.minibatch]
                    perm_h[mb:mb + cfg.minibatch] = part[np.argsort(widths[part], kind="stable")]
                perm = torch.from_numpy(perm_h).to(self.dev)
            for mb in range(0, rows, cfg.minibatch):
                idx_mb = perm[mb:mb + cfg.minibatch]
                self.opt.zero_grad(set_to_none=True)
                for mc, me in self._micros(widths[perm_h[mb:mb + idx_mb.shape[0]]]):
                    idx = idx_mb[mc:me]
                    rows_h = perm_h[mb + mc:mb + me]
                    t = int(widths[rows_h].max())
                    frac = idx.shape[0] / idx_mb.shape[0]
                    tok, ptok = self._micro_in(flat, idx, t, rows_h)
                    if may_y is not None:
                        tok["y_rows"] = E2ENet.y_rows_of(may_y[rows_h], self.dev)
                    with self._autocast():
                        logits, v = self.net(tok, ptok)
                    logp_all, ent = masked_stats(logits.float())
                    logp = logp_all.gather(1, flat["action"][idx].unsqueeze(1)).squeeze(1)
                    ratio = torch.exp(logp - flat["logp"][idx])
                    a = adv[idx]
                    s1 = ratio * a
                    s2 = ratio.clamp(1 - cfg.clip, 1 + cfg.clip) * a
                    obj = torch.min(s1, s2)
                    obj = torch.where(a < 0, torch.max(obj, cfg.dual_clip * a), obj)
                    c = combat[idx]
                    coef = torch.where(c, torch.full_like(ent, ent_c), torch.full_like(ent, ent_r))
                    if gym_w is None:
                        vloss = 0.5 * (v.float() - flat["ret"][idx]).pow(2).mean()
                        pg, ent_term = obj.mean(), (coef * ent).mean()
                    else:  # RLN43: the gym rows weighted by `practice.human.loss_weight`
                        w = torch.where(flat["gym"][idx], gym_w, 1.0)
                        vloss = 0.5 * (w * (v.float() - flat["ret"][idx]).pow(2)).mean()
                        pg, ent_term = (w * obj).mean(), (w * coef * ent).mean()
                    if v_only:
                        loss = cfg.vf_coef * vloss
                    else:
                        loss = -pg - ent_term + cfg.vf_coef * vloss
                    if use_prior:  # RLN42: beta * KL(pi_M || prior) on mapped run rows
                        kl_rows, kind_kl = self.prior.kl(logp_all, flat["pent"][idx], beta)
                        loss = loss + kl_rows.mean()
                        pkl.add_kl(kind_kl)
                    if kt is not None and (ex_on or epochs_done == 0):
                        loss = loss + self._kind_terms(
                            kt, ex, ex_c, ex_on, logp_all, ~c, flat, idx, epochs_done == 0,
                            k_acc, k_named, smith)
                    (loss * frac).backward()
                    with torch.no_grad():
                        acc += torch.stack([
                            -obj.detach().double().mean(), vloss.detach().double(),
                            ((ratio.detach() - 1).abs() > cfg.clip).double().mean()]) * frac
                        ent_acc.add(ent, c)
                n_mb += 1
                torch.nn.utils.clip_grad_norm_(self.net.parameters(), cfg.grad_clip)
                self.opt.step()
            epochs_done += 1
            kl = self._kl(old_probe) if old_probe is not None else float("nan")
            kls.append(kl)
            kls_b.append(self._kl(old_batch, self._batch_logp(flat, brows, bwidths, y_probe)))
            if (kls_b if cfg.kl_source == "batch" else kls)[-1] > cfg.kl_stop:
                break
        kl = kls[-1]
        kl_ctl = kls_b[-1] if cfg.kl_source == "batch" else kl
        lo, hi = cfg.lr_factor_range
        if v_only:
            pass  # the policy did not move: its KL says nothing about the lr
        elif kl_ctl < cfg.kl_low:
            self.lr_factor = min(hi, self.lr_factor * 1.5)
        elif kl_ctl > cfg.kl_high:
            self.lr_factor = max(lo, self.lr_factor / 1.5)
        with torch.no_grad():
            params = list(self.net.parameters())
            for decay, ema in zip(cfg.ema, self.emas):
                torch._foreach_lerp_(ema, [p.detach() for p in params], 1 - decay)
        n = max(n_mb, 1)
        out = dict(zip(("pg", "v", "clipfrac"), (acc / n).tolist()))
        out["ent_combat"], out["ent_run"] = ent_acc.means()
        if mass is not None:  # RLN43: row shares and the weighted gradient mass (run rows)
            out["gym_mass"] = mass
        if kt is not None:
            out["explore"] = self._kind_log(ex, ex_c, ex_on, k_acc, k_named, smith)
        if cfg.ent_boost and not v_only:
            ent_boost_step(cfg, (out["ent_combat"], out["ent_run"]), self.ent_mult,
                           self.ent_streak, self.ent_boosts)
        v, ret = flat["value"], flat["ret"]
        finite = bool(torch.stack([torch.isfinite(p).all() for p in params]).all())
        out.update(kl=kl, kls=kls, kl_batch=kls_b[-1], kls_batch=kls_b, epochs=epochs_done, lr_factor=self.lr_factor,
                   ev_combat=explained_variance(v[combat], ret[combat]),
                   ev_run=explained_variance(v[~combat], ret[~combat]),
                   finite=finite and math.isfinite(out["v"]) and math.isfinite(out["pg"]),
                   ent_coef=[ent_c, ent_r], ent_mult=list(self.ent_mult), combat_share=float(combat.float().mean()),
                   **(dict(v_only=True) if v_only else {}))
        if use_prior:
            out["prior_kl"], out["prior_beta"] = pkl.kl_means(), beta
        out.update(buf.get("ret_stats", {}))
        out["rows_empty"] = empty
        return out

    def _kind_terms(self, kt, ex, ex_c, ex_on, logp_all, run, flat, idx, measure, k_acc,
                    k_named, smith):
        """RLN43: the explore-floor KL of the kinds on (a loss term, 0.0 if none) and, in the
        first epoch, the kinds' entropy, KL and named P sums and P(Smith) by HP band."""
        term = 0.0
        for kind in EXPLORE_KINDS:
            if kind not in ex_on and not measure:
                continue
            on, lq, legal, n, frac = kt.stats(kind, logp_all, run)
            kl = explore_kl(lq, legal, n, on, ex.direction if ex else "pi_u")
            if kind in ex_on:
                term = term + ex_c * kl.mean()
            if measure:
                with torch.no_grad():
                    k_acc[kind] += torch.stack([frac.sum(), on.sum(), kl.sum()]).double()
                    for nm, (x, m) in kt.named_p(kind, logp_all, on).items():
                        k_named[kind][nm] += torch.stack([x, m]).double()
                    if kind == "rest" and "rest" in flat:
                        rr = flat["rest"][idx].long()
                        has = on & (rr >= 0)
                        p = lq.gather(1, rr.clamp(min=0).unsqueeze(1)).squeeze(1).exp()
                        b = flat["rest_hp"][idx].long()
                        smith[:, 0].index_add_(0, b, (p * has).double())
                        smith[:, 1].index_add_(0, b, has.double())
        return term

    def _kind_log(self, ex, ex_c, ex_on, k_acc, k_named, smith):
        """RLN43: the log's `explore` record; keeps the entropy fractions for the next switch."""
        kinds = {}
        for kind, acc in k_acc.items():
            e, rows, kl = acc.tolist()
            rec = dict(ent=e / rows if rows else None, rows=int(rows),
                       kl=kl / rows if rows else None)
            for nm, v in k_named[kind].items():
                x, m = v.tolist()
                rec["p_" + nm] = x / m if m else None
            kinds[kind] = rec
        a = smith.cpu().numpy()
        names = ("<30", "30-49", "50-69", "70-89", ">=90")
        kinds["rest"]["p_smith"] = float(a[:, 0].sum() / a[:, 1].sum()) if a[:, 1].sum() else None
        kinds["rest"]["smith_bands"] = {nm: [float(x / m) if m else None, int(m)]
                                        for nm, (x, m) in zip(names, a)}
        self.kind_ent = {k: v["ent"] for k, v in kinds.items()}
        out = dict(kinds=kinds)
        if ex is not None:
            out.update(coef=ex_c, on=ex_on)
        return out

    # ---- loop ----

    def _sync(self):
        if self.dev.type == "cuda":
            torch.cuda.current_stream().synchronize()

    def iterate(self):
        t0 = time.perf_counter()
        if not self.cfg.overlap:
            buf, runs = self.collect()
            self._sync()
            t1 = time.perf_counter()
            upd = self.update(buf)
            self._sync()
            t2 = time.perf_counter()
            collect_s, update_s = t1 - t0, t2 - t1
        else:
            if self.pending is None:  # a start or resume: this batch is collected first
                self._practice_apply()
                pos = self._position()
                ahead = dict(buf=self._alloc())
                self._order_collect()
                self._collect_ahead(ahead)
                if "error" in ahead:
                    raise ahead["error"]
                self.pending = (ahead["buf"], ahead["runs"], ahead["secs"], pos)
            buf, runs, collect_s, _ = self.pending
            self._finish(buf)
            # The next batch: the weights before this update, the position after the last.
            self._copy_behaviour()
            self._practice_apply()
            pos = self._position()
            ahead = dict(buf=self._alloc())
            self._order_collect()
            worker = threading.Thread(target=self._collect_ahead, args=(ahead,), daemon=True)
            worker.start()
            t1 = time.perf_counter()
            try:
                upd = self.update(buf)
                self._sync()
                update_s = time.perf_counter() - t1
            finally:
                worker.join()
            if "error" in ahead:
                self.pending = None
                raise ahead["error"]
            self.pending = (ahead["buf"], ahead["runs"], ahead["secs"], pos)
            collect_s = ahead["secs"]
        t2 = time.perf_counter()
        rows = self.cfg.envs * self.cfg.steps
        self.decisions += rows
        self.iteration += 1
        floors = runs["floors"]
        if "deaths" in runs:
            self._adaptive_update(runs["deaths"])
        rec = dict(
            iteration=self.iteration, decisions=self.decisions,
            collect_dps=rows / collect_s, update_dps=rows / update_s,
            total_dps=rows / (t2 - t0),
            runs=len(floors), mean_floor=float(np.mean(floors)) if floors else None,
            win_rate=runs["wins"] / len(floors) if floors else None, ends=runs["ends"],
            caps=sum(runs["ends"].get(k, 0) for k in ("capped", "fight_capped",
                                                      "decision_capped")),
            tokens=runs["tokens"], overflow=runs["overflow"], empty_ends=runs["empty"],
            discards=runs["discards"], discard_offers=runs["offers"],
            reward_sum=float(buf["reward"].sum()),
            gpu_mem_gb=(torch.cuda.max_memory_allocated() / 2**30
                        if self.dev.type == "cuda" else 0.0),
            **upd)
        if "prior" in runs:
            rec["prior"] = runs["prior"]
        if "hp_reward" in runs:
            rec["hp_reward"] = runs["hp_reward"]
        if "gym" in runs:  # RLN43: the gym's rows, rewards, starts and fights by cell
            g = dict(runs["gym"])
            if self.cfg.practice.gym() is not None:
                st = self.envs.gym_stats()
                g["started"] = dict(zip(GYM_CELL_NAMES, st["started"]))
                g["empty"] = sum(st["empty"])
            pr = runs.get("practice", {})
            gb = [v for k, v in pr.items() if k.startswith("GYM_BOSS")]
            ge = [v for k, v in pr.items() if k.startswith("GYM_ELITE")]
            g["boss_win"] = sum(v[1] for v in gb) / sum(v[0] for v in gb) if gb else None
            g["elite_win"] = sum(v[1] for v in ge) / sum(v[0] for v in ge) if ge else None
            rec["gym"] = g
        if self.telemetry is not None:
            rec["behaviour"] = self.telemetry.record()
        if "timing" in runs:
            rec["collect_timing"] = runs["timing"]
        if "practice" in runs:
            st = self.envs.practice_stats()
            pr = dict(runs["practice"])
            ends, kills = pr.pop("ends", {}), pr.pop("kills", {})
            boss = [v for k, v in pr.items() if k.startswith("BOSS")]
            rec["practice"] = dict(
                slots=self.cfg.practice.schedule(self.decisions, self.cfg.envs,
                                                 self.adapt_loss)[0],
                ended=pr, ends=ends, kills=kills, pool=dict(zip(CELL_NAMES, st["pool"])),
                started=dict(zip(CELL_NAMES, st["started"])),
                to_run=sum(st["to_run"]),
                **(dict(adaptive=dict(
                    loss=dict(zip(CELL_NAMES, self.adapt_loss)),
                    weights=dict(zip(CELL_NAMES, (w / 1e6 for w in
                                      self.cfg.practice.schedule(
                                          self.decisions, self.cfg.envs, self.adapt_loss)[1]))),
                    deaths=runs["deaths"])) if "deaths" in runs else {}),
                boss_win=(sum(v[1] for v in boss) / sum(v[0] for v in boss)) if boss else None)
        with open(self.out / "stats.jsonl", "a") as f:
            f.write(json.dumps(rec) + "\n")
        return rec

    def close(self):
        self.envs.close()


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", required=True)
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--minutes", type=float, default=None)
    ap.add_argument("--iterations", type=int, default=None)
    ap.add_argument("--decisions", type=int, default=None)
    ap.add_argument("--config", default=None, help="a JSON file of Config overrides")
    ap.add_argument("--set", action="append", default=[], help="key=json overrides")
    args = ap.parse_args(argv)
    out = pathlib.Path(args.out).expanduser()
    if args.resume:
        cfg = Config.from_dict(torch.load(out / "state.pt", weights_only=False)["config"])
        for kv in args.set:  # speed flags only: a resume must keep the learning settings
            k, v = kv.split("=", 1)
            if k not in RESUME_SETTABLE:
                ap.error(f"--set {k} is not allowed with --resume")
            set_resumed(cfg, k, json.loads(v))
    else:
        over = json.loads(pathlib.Path(args.config).read_text()) if args.config else {}
        for kv in args.set:
            k, v = kv.split("=", 1)
            over[k] = json.loads(v)
        cfg = Config.from_dict({**dataclasses.asdict(Config()), **over})
    learner = Learner(cfg, out, resume=args.resume)
    (out / "config.json").write_text(json.dumps(dataclasses.asdict(cfg), indent=1))
    t0 = time.perf_counter()
    last_ckpt = learner.decisions
    try:
        while True:
            rec = learner.iterate()
            print(f"it {rec['iteration']:4d} dec {rec['decisions'] / 1e6:7.2f}M "
                  f"floor {rec['mean_floor'] or 0:5.2f} runs {rec['runs']:5d} "
                  f"dps {rec['collect_dps']:7.0f}/{rec['update_dps']:6.0f}/{rec['total_dps']:6.0f} "
                  f"kl {rec['kl']:.4f} klb {rec['kl_batch']:.4f} m {rec['lr_factor']:.2f} v {rec['v']:.4f} "
                  f"ent {rec['ent_combat']:.3f}/{rec['ent_run']:.3f} "
                  f"ev {rec['ev_combat']:.2f}/{rec['ev_run']:.2f} tok {rec['tokens']:.1f} "
                  f"mem {rec['gpu_mem_gb']:.1f}G", flush=True)
            if not rec["finite"]:
                print("non-finite: stopping", flush=True)
                break
            if learner.decisions - last_ckpt >= cfg.ckpt_every:
                learner.save()
                last_ckpt = learner.decisions
            if args.minutes and time.perf_counter() - t0 > args.minutes * 60:
                break
            if args.iterations and learner.iteration >= args.iterations:
                break
            if args.decisions and learner.decisions >= args.decisions:
                break
    finally:
        if learner.compiled:
            from torch._dynamo.utils import counters
            print(f"dynamo: frames {dict(counters['frames'])} "
                  f"unique_graphs {counters['stats']['unique_graphs']} "
                  f"recompiles {sum(counters['recompiles'].values()) if 'recompiles' in counters else 0}",
                  flush=True)
        learner.save()
        np.save(out / "floors.npy", np.asarray(learner.floors, dtype=np.int16))
        learner.close()


if __name__ == "__main__":
    main()
