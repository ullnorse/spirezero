//! CB2: the combat search of `python/cb1_mcts.py` (`MCTSPlayer`, leaf = value) in Rust, so the
//! tree, the determinized copies and their steps run on the engine's threads and the net is
//! called once per round on every fight's leaves together.
//!
//! **Protocol.** [`Search::begin`] takes the fights to search (main envs of a [`RunEnvs`]), their
//! root logits and priors and the random numbers the Python player would draw (Gumbel noise and
//! world seeds), and copies each main env once (its source for the search's copies). Then, until
//! it returns 0, [`Search::run`] advances every fight to its next round of leaves and writes their
//! observations, masks and privileged buffers into the caller's request buffers; the caller
//! evaluates them with the net and hands the logits, priors and values to [`Search::feed`].
//! [`Search::results`] gives each fight's move and improved policy; after the real step,
//! [`Search::after_step`] keeps the subtree below the move played when the real observation is
//! one of its chance children.
//!
//! **Same play as the Python player.** Every rule of `cb1_mcts.MCTSPlayer` is kept: Gumbel-top-k
//! root candidates, sequential halving over `ceil(log2 m)` phases with the same budget split,
//! waves of `wave` simulations per fight with a virtual loss, PUCT below the root with an
//! unvisited action's Q completed by the node's mean, children keyed by (action, observation and
//! mask), leaves valued by `V + hp_coef * HP/maxHP + pot_w * potions` and fight ends by
//! `won * (1 + hp_coef * HP/maxHP + pot_w * potions)`, a simulation dropped when its world forbids
//! the root move. Sums over a node's actions follow numpy's pairwise order and the caller supplies
//! the softmax priors, so a fight given the same net outputs takes the same decisions (the
//! identity check in `python/cb2_search.py`). The simulations of one wave descend in lockstep by
//! depth, as the Python waves do; their leaves are evaluated at the end of the wave, which changes
//! nothing, since a node at depth `d` is only reached at depth `d`.
//!
//! **Honesty.** A simulation runs on a determinized copy of the fight (`World::Fresh`,
//! `World::Paired`, `World::One`); the value reads the copy's privileged buffer, never the real
//! fight's. `World::Exact` copies the real hidden state: a diagnostic ceiling only.

#![allow(
    clippy::float_arithmetic,
    reason = "search statistics (visit counts, values, priors), not game logic"
)]
#![allow(
    clippy::suboptimal_flops,
    reason = "the Python player's rounding: separate multiply and add, as numpy does"
)]

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Counts, RunConfig, RunEnd, RunEnv, RunEnvs, Slots};
use crate::priv_obs::PRIV_BYTES;
use crate::run_action::{RUN_ACTIONS, RUN_MASK_WORDS};
use crate::run_obs::{BELT_AT, RUN_OBS_BYTES};

/// The run header's phase byte and its combat code (`run_obs::phase_code`).
const PHASE_AT: usize = 3;
const PHASE_COMBAT: u8 = 3;
/// The run header's HP and max HP (`i16`).
const HP_AT: usize = 8;
const MAX_HP_AT: usize = 10;
/// Potion slots on the belt (2 bytes each).
const BELT_SLOTS: usize = 6;

/// Where a simulation's world comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// A fresh determinization per simulation.
    Fresh,
    /// Per decision, simulation `j` of every root move uses world seed `j` (common random
    /// numbers).
    Paired,
    /// One determinization per decision.
    One,
    /// The real hidden state (dishonest: a diagnostic ceiling).
    Exact,
}

/// The search's settings (`cb1_mcts.MCTSPlayer`'s).
#[derive(Clone, Copy, Debug)]
pub struct SearchConfig {
    /// Simulations per decision.
    pub sims: usize,
    /// Root candidates.
    pub m: usize,
    /// Simulations per fight per round.
    pub wave: usize,
    /// PUCT's exploration weight.
    pub c_puct: f64,
    /// Gumbel `MuZero`'s `c_visit` and `c_scale`.
    pub c_visit: f64,
    pub c_scale: f64,
    /// Plies per simulation before the value is used.
    pub depth: usize,
    /// Utility weights of the HP fraction and of each potion kept.
    pub hp_coef: f64,
    pub pot_w: f64,
    pub world: World,
    /// Keep the subtree below the move played.
    pub keep_tree: bool,
    /// Worker threads.
    pub threads: usize,
}

/// Counters over the search's life (`cb1_mcts.MCTSPlayer.stats`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStats {
    pub searched: u64,
    pub reused: u64,
    pub dropped_sims: u64,
    pub illegal_sim: u64,
    pub sim_steps: u64,
    pub sim_ends: u64,
    pub leaf_evals: u64,
    pub dead_end: u64,
    pub sims: u64,
    pub rounds: u64,
}

impl SearchStats {
    /// The counters in a fixed order (the FFI's).
    #[must_use]
    pub const fn to_array(&self) -> [u64; 10] {
        [
            self.searched,
            self.reused,
            self.dropped_sims,
            self.illegal_sim,
            self.sim_steps,
            self.sim_ends,
            self.leaf_evals,
            self.dead_end,
            self.sims,
            self.rounds,
        ]
    }

    const fn add(&mut self, o: &Self) {
        self.searched += o.searched;
        self.reused += o.reused;
        self.dropped_sims += o.dropped_sims;
        self.illegal_sim += o.illegal_sim;
        self.sim_steps += o.sim_steps;
        self.sim_ends += o.sim_ends;
        self.leaf_evals += o.leaf_evals;
        self.dead_end += o.dead_end;
        self.sims += o.sims;
        self.rounds += o.rounds;
    }
}

/// numpy's sum of a float64 array (`pairwise_sum`: blocks of up to 128 with eight accumulators,
/// halved above that), so a node's sums round as the Python player's do.
fn np_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0;
        for &x in a {
            r += x;
        }
        r
    } else if n <= 128 {
        let mut r = [0.0; 8];
        r.copy_from_slice(&a[..8]);
        let full = n - n % 8;
        let mut i = 8;
        while i < full {
            for (j, acc) in r.iter_mut().enumerate() {
                *acc += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for &x in &a[i..] {
            res += x;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        np_sum(&a[..n2]) + np_sum(&a[n2..])
    }
}

/// A 64-bit key of a position: its observation and mask (`cb1_mcts.key_of`).
fn key_of(obs: &[u8], mask: &[u64]) -> u64 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut h: u64 = 0x2545_F491_4F6C_DD1D;
    let mix = |h: u64, w: u64| (h ^ w).wrapping_mul(K).rotate_left(29);
    let (words, rest) = obs.as_chunks::<8>();
    for w in words {
        h = mix(h, u64::from_le_bytes(*w));
    }
    let mut tail = [0_u8; 8];
    tail[..rest.len()].copy_from_slice(rest);
    h = mix(h, u64::from_le_bytes(tail) ^ (rest.len() as u64) << 56);
    for &w in mask {
        h = mix(h, w);
    }
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ h >> 29
}

/// Whether mask words `mask` hold action `a`.
fn has(mask: &[u64], a: u16) -> bool {
    let a = usize::from(a);
    mask.get(a / 64).is_some_and(|w| w >> (a % 64) & 1 == 1)
}

fn i16_at(obs: &[u8], at: usize) -> f64 {
    f64::from(i16::from_le_bytes([obs[at], obs[at + 1]]))
}

/// HP / max HP of the run header (`cb1_mcts.phi`).
fn phi(obs: &[u8]) -> f64 {
    i16_at(obs, HP_AT).max(0.0) / i16_at(obs, MAX_HP_AT).max(1.0)
}

/// Potions on the belt (`naz3_combat.belt_counts`).
fn belts(obs: &[u8]) -> f64 {
    let n = obs[BELT_AT..BELT_AT + 2 * BELT_SLOTS]
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|p| p[0] != 0 || p[1] != 0)
        .count();
    f64::from(u8::try_from(n).unwrap_or(u8::MAX))
}

type NodeId = u32;

/// A visible position: per legal action (ascending), its logit, prior, visits and value sum.
#[derive(Debug, Default)]
struct Node {
    acts: Vec<u16>,
    logits: Vec<f64>,
    prior: Vec<f64>,
    n: Vec<f64>,
    w: Vec<f64>,
    /// (action, key of the observation it led to, child).
    children: Vec<(u16, u64, NodeId)>,
    /// The node's own leaf utility (`None` for a root never evaluated as a leaf).
    v: Option<f64>,
}

impl Node {
    /// A node of the legal actions of `logits` (the finite ones), with their `prior`.
    fn new(logits: &[f64], prior: &[f64], v: Option<f64>) -> Self {
        let mut node = Self {
            v,
            ..Self::default()
        };
        node.set_logits(logits, prior);
        node.n = vec![0.0; node.acts.len()];
        node.w = vec![0.0; node.acts.len()];
        node
    }

    /// New logits and priors for the same legal actions (a reused root).
    fn set_logits(&mut self, logits: &[f64], prior: &[f64]) {
        self.acts.clear();
        self.logits.clear();
        self.prior.clear();
        for (a, (&l, &p)) in logits.iter().zip(prior).enumerate() {
            if l.is_finite() {
                self.acts
                    .push(u16::try_from(a).expect("fewer than 65,536 actions"));
                self.logits.push(l);
                self.prior.push(p);
            }
        }
    }

    fn slot(&self, a: u16) -> Option<usize> {
        self.acts.binary_search(&a).ok()
    }

    fn child(&self, a: u16, h: u64) -> Option<NodeId> {
        self.children
            .iter()
            .find(|&&(ca, ch, _)| ca == a && ch == h)
            .map(|&(_, _, id)| id)
    }

    /// The sum of `x` over the full action width (zeros at the illegal actions), as numpy sums
    /// a node's array.
    fn wide_sum(&self, x: &[f64], width: usize, scratch: &mut Vec<f64>) -> f64 {
        scratch.clear();
        scratch.resize(width, 0.0);
        for (&a, &v) in self.acts.iter().zip(x) {
            scratch[usize::from(a)] = v;
        }
        np_sum(scratch)
    }

    fn visits(&self, width: usize, scratch: &mut Vec<f64>) -> f64 {
        self.wide_sum(&self.n, width, scratch)
    }

    /// `Node.mean`: the mean backed-up value, else the own estimate, else 0.
    fn mean(&self, width: usize, scratch: &mut Vec<f64>) -> f64 {
        let n = self.visits(width, scratch);
        if n > 0.0 {
            self.wide_sum(&self.w, width, scratch) / n
        } else {
            self.v.unwrap_or(0.0)
        }
    }

    /// `MCTSPlayer.puct`: the action maximising `Q + c_puct * P * sqrt(N + 1) / (1 + N(a))`.
    fn puct(&self, c_puct: f64, width: usize, scratch: &mut Vec<f64>) -> u16 {
        let total = self.visits(width, scratch);
        let mean = if self.n.iter().any(|&n| n <= 0.0) {
            self.mean(width, scratch)
        } else {
            0.0
        };
        let root = (total + 1.0).sqrt();
        let mut best = (f64::NEG_INFINITY, None);
        for k in 0..self.acts.len() {
            let q = if self.n[k] > 0.0 {
                self.w[k] / self.n[k].max(1.0)
            } else {
                mean
            };
            let s = q + c_puct * self.prior[k] * root / (1.0 + self.n[k]);
            if best.1.is_none() || s > best.0 {
                best = (s, Some(k));
            }
        }
        self.acts[best.1.expect("a node with a legal action")]
    }

    /// `MCTSPlayer.completed_q` (per legal action, min-max normalised).
    fn completed_q(&self, width: usize, scratch: &mut Vec<f64>) -> Vec<f64> {
        let mut q: Vec<f64> = (0..self.acts.len())
            .map(|k| {
                if self.n[k] > 0.0 {
                    self.w[k] / self.n[k].max(1.0)
                } else {
                    0.0
                }
            })
            .collect();
        let vis: Vec<usize> = (0..self.acts.len()).filter(|&k| self.n[k] > 0.0).collect();
        let mix = if vis.is_empty() {
            self.mean(width, scratch)
        } else {
            let pq: Vec<f64> = vis.iter().map(|&k| self.prior[k] * q[k]).collect();
            let pv: Vec<f64> = vis.iter().map(|&k| self.prior[k]).collect();
            np_sum(&pq) / np_sum(&pv).max(1e-12)
        };
        for (x, &n) in q.iter_mut().zip(&self.n) {
            if n <= 0.0 {
                *x = mix;
            }
        }
        let lo = q.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = q.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let span = (hi - lo).max(1e-6);
        q.iter().map(|&x| (x - lo) / span).collect()
    }

    fn max_visits(&self) -> f64 {
        self.n.iter().copied().fold(0.0, f64::max)
    }
}

/// One simulation's scratch: a copy of the fight and its slots, its path and utility.
#[derive(Debug)]
struct Sim {
    env: RunEnv,
    obs: Vec<u8>,
    privs: Vec<u8>,
    mask: Vec<u64>,
    counts: Vec<u8>,
    path: Vec<(NodeId, u16)>,
    util: f64,
}

impl Sim {
    fn new(config: RunConfig) -> Self {
        Self {
            env: RunEnv::new(config),
            obs: vec![0; RUN_OBS_BYTES],
            privs: vec![0; PRIV_BYTES],
            mask: vec![0; RUN_MASK_WORDS],
            counts: vec![0; RUN_ACTIONS],
            path: Vec::new(),
            util: 0.0,
        }
    }

    fn slots(&mut self) -> (&mut RunEnv, Slots<'_>) {
        (
            &mut self.env,
            Slots {
                obs: &mut self.obs,
                privs: &mut self.privs,
                mask: &mut self.mask,
                counts: &mut self.counts,
            },
        )
    }

    /// Whether the fight is over in this world (the episode ended, or it left combat).
    fn finished(&self) -> bool {
        self.env.end != RunEnd::None || self.obs[PHASE_AT] != PHASE_COMBAT
    }

    /// `MCTSPlayer.end_util` of a finished simulation.
    fn end_util(&self, hp_coef: f64, pot_w: f64) -> f64 {
        let won = matches!(
            self.env.end,
            RunEnd::None | RunEnd::GymEnd | RunEnd::Victory
        );
        if won {
            1.0 + hp_coef * phi(&self.obs) + pot_w * belts(&self.obs)
        } else {
            0.0
        }
    }
}

/// A leaf awaiting the net: simulation `sim`, reached by `action` from `node`, key `key`.
#[derive(Clone, Copy, Debug)]
struct Need {
    sim: usize,
    node: NodeId,
    action: u16,
    key: u64,
}

/// A fed leaf: its logits and priors (the full width) and value.
#[derive(Debug, Default)]
struct Fed {
    logits: Vec<f64>,
    prior: Vec<f64>,
    value: f64,
}

/// One fight's search state.
#[derive(Debug)]
struct Fight {
    config: RunConfig,
    /// A copy of the main env at the decision (every simulation copies it).
    source: RunEnv,
    nodes: Vec<Node>,
    root: NodeId,
    /// The kept root and its position's key, between decisions.
    kept: Option<(NodeId, u64)>,
    /// The root and the move played, until the real step.
    played: Option<(NodeId, u16)>,
    gumbel: Vec<f64>,
    seeds: Vec<u64>,
    /// Per action, the simulations it has had this decision (paired worlds).
    wcnt: Vec<usize>,
    /// `SplitMix64` state for fresh worlds.
    rng: u64,
    cands: Vec<u16>,
    phases: usize,
    phase: usize,
    used: usize,
    /// The current phase's simulations left (root moves), and whether a phase is under way.
    todo: std::collections::VecDeque<u16>,
    in_phase: bool,
    active: bool,
    done: bool,
    /// The current wave's simulations and leaves.
    wave: usize,
    needs: Vec<Need>,
    fed: Vec<Fed>,
    sims: Vec<Sim>,
    pick: u16,
    target: Vec<(u16, f64)>,
    visits: f64,
    stats: SearchStats,
    scratch: Vec<f64>,
}

impl Fight {
    fn new(config: RunConfig) -> Self {
        Self {
            config,
            source: RunEnv::new(config),
            nodes: Vec::new(),
            root: 0,
            kept: None,
            played: None,
            gumbel: Vec::new(),
            seeds: Vec::new(),
            wcnt: Vec::new(),
            rng: 0,
            cands: Vec::new(),
            phases: 0,
            phase: 0,
            used: 0,
            todo: std::collections::VecDeque::new(),
            in_phase: false,
            active: false,
            done: true,
            wave: 0,
            needs: Vec::new(),
            fed: Vec::new(),
            sims: Vec::new(),
            pick: 0,
            target: Vec::new(),
            visits: 0.0,
            stats: SearchStats::default(),
            scratch: Vec::new(),
        }
    }

    fn add(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        NodeId::try_from(self.nodes.len() - 1).expect("fewer than 2^32 nodes")
    }

    fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }

    /// Keeps only the subtree under `root` (renumbered, `root` first).
    fn compact(&mut self, root: NodeId) -> NodeId {
        let mut old = std::mem::take(&mut self.nodes);
        let mut map = vec![NodeId::MAX; old.len()];
        let mut order = vec![root];
        map[root as usize] = 0;
        let mut i = 0;
        while i < order.len() {
            let id = order[i];
            for &(_, _, c) in &old[id as usize].children {
                if map[c as usize] == NodeId::MAX {
                    map[c as usize] = NodeId::try_from(order.len()).expect("few nodes");
                    order.push(c);
                }
            }
            i += 1;
        }
        self.nodes = order
            .iter()
            .map(|&id| {
                let mut n = std::mem::take(&mut old[id as usize]);
                for c in &mut n.children {
                    c.2 = map[c.2 as usize];
                }
                n
            })
            .collect();
        0
    }

    const fn next_seed(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) >> 1
    }

    /// The sort key of root move `a` (`g + logits + sigma(completed Q)`).
    fn scores(&mut self, cfg: &SearchConfig, width: usize) -> Vec<f64> {
        let mut scratch = std::mem::take(&mut self.scratch);
        let root = self.node(self.root);
        let cq = root.completed_q(width, &mut scratch);
        let scale = (cfg.c_visit + root.max_visits()) * cfg.c_scale;
        let mut out = vec![f64::NEG_INFINITY; width];
        for (k, &a) in root.acts.iter().enumerate() {
            let a = usize::from(a);
            out[a] = self.gumbel[a] + root.logits[k] + scale * cq[k];
        }
        self.scratch = scratch;
        out
    }

    /// Advances the fight until its current wave needs the net, or its search is over.
    fn advance(&mut self, cfg: &SearchConfig, width: usize) {
        if !self.active {
            return;
        }
        if !self.fed.is_empty() {
            self.finish_wave(cfg);
        }
        loop {
            if self.todo.is_empty() {
                if self.in_phase {
                    let s = self.scores(cfg, width);
                    // Python's `list.sort` is stable: ties keep the candidates' order
                    self.cands
                        .sort_by(|&a, &b| s[usize::from(b)].total_cmp(&s[usize::from(a)]));
                    let keep = (self.cands.len() / 2).max(1);
                    self.cands.truncate(keep);
                    self.in_phase = false;
                    self.phase += 1;
                }
                if self.phase >= self.phases || self.cands.len() <= 1 {
                    self.conclude(cfg, width);
                    return;
                }
                let c = self.cands.len();
                let share =
                    c.max((cfg.sims.saturating_sub(self.used)) / (self.phases - self.phase));
                let per = (share / c).max(1);
                for &a in &self.cands {
                    for _ in 0..per {
                        self.todo.push_back(a);
                    }
                }
                self.used += per * c;
                self.in_phase = true;
            }
            let n = self.todo.len().min(cfg.wave);
            let wave: Vec<u16> = self.todo.drain(..n).collect();
            self.simulate(cfg, &wave, width);
            if !self.needs.is_empty() {
                return;
            }
            self.backup();
        }
    }

    /// Starts a wave of simulations (root moves `wave`): copies, then lockstep descent by depth
    /// until each simulation ends, is dropped, or reaches a leaf (queued in `needs`).
    fn simulate(&mut self, cfg: &SearchConfig, wave: &[u16], width: usize) {
        while self.sims.len() < wave.len() {
            self.sims.push(Sim::new(self.config));
        }
        self.wave = wave.len();
        self.needs.clear();
        self.stats.sims += wave.len() as u64;
        let root = self.root;
        let mut live = Vec::with_capacity(wave.len());
        for (j, &a) in wave.iter().enumerate() {
            let slot = self.node(root).slot(a).expect("a legal root move");
            self.node_mut(root).n[slot] += 1.0;
            let seed = match cfg.world {
                World::Exact => None,
                World::Fresh => Some(self.next_seed()),
                World::Paired | World::One => {
                    let k = self.wcnt[usize::from(a)];
                    self.wcnt[usize::from(a)] += 1;
                    Some(self.seeds[k % self.seeds.len()])
                }
            };
            let sim = &mut self.sims[j];
            sim.path.clear();
            sim.path.push((root, a));
            sim.util = 0.0;
            sim.env.copy_from(&self.source, seed);
            let (env, mut slots) = sim.slots();
            env.settle_copy(&mut slots);
            if has(&sim.mask, a) {
                live.push(j);
            } else {
                // the world's mask differs from the real fight's: drop the simulation
                self.stats.dropped_sims += 1;
                self.sims[j].path.clear();
                self.node_mut(root).n[slot] -= 1.0;
            }
        }
        let mut depth = 0;
        let mut scratch = std::mem::take(&mut self.scratch);
        while !live.is_empty() {
            let mut next = Vec::with_capacity(live.len());
            let mut stepped = Vec::with_capacity(live.len());
            for &j in &live {
                let &(node, a) = self.sims[j].path.last().expect("a live path");
                if has(&self.sims[j].mask, a) {
                    stepped.push(j);
                    continue;
                }
                // a move this world forbids: the simulation ends at the position reached
                self.stats.illegal_sim += 1;
                self.sims[j].path.pop();
                let n = self.node_mut(node);
                let slot = n.slot(a).expect("a legal move");
                n.n[slot] -= 1.0;
                if !self.sims[j].path.is_empty() {
                    let n = self.node(node);
                    self.sims[j].util = n.v.unwrap_or_else(|| n.mean(width, &mut scratch));
                }
            }
            depth += 1;
            for &j in &stepped {
                let &(node, a) = self.sims[j].path.last().expect("a live path");
                let sim = &mut self.sims[j];
                let (env, mut slots) = sim.slots();
                let _: Counts = env.step(a, &mut slots);
                self.stats.sim_steps += 1;
                if sim.finished() {
                    sim.util = sim.end_util(cfg.hp_coef, cfg.pot_w);
                    self.stats.sim_ends += 1;
                    continue;
                }
                let key = key_of(&sim.obs, &sim.mask);
                let child = self.node(node).child(a, key);
                let leaf = match child {
                    None => true,
                    Some(c) => {
                        let dead = self.node(c).acts.is_empty();
                        if dead {
                            self.stats.dead_end += 1;
                        }
                        depth >= cfg.depth || dead
                    }
                };
                if leaf {
                    self.needs.push(Need {
                        sim: j,
                        node,
                        action: a,
                        key,
                    });
                    continue;
                }
                let c = child.expect("an inner child");
                let b = self.node(c).puct(cfg.c_puct, width, &mut scratch);
                let cn = self.node_mut(c);
                let slot = cn.slot(b).expect("a legal move");
                cn.n[slot] += 1.0;
                self.sims[j].path.push((c, b));
                next.push(j);
            }
            live = next;
        }
        self.scratch = scratch;
    }

    /// Expands the fed leaves (in the order they were reached) and backs the wave up.
    fn finish_wave(&mut self, cfg: &SearchConfig) {
        let needs = std::mem::take(&mut self.needs);
        let fed = std::mem::take(&mut self.fed);
        for (need, f) in needs.iter().zip(&fed) {
            let obs = &self.sims[need.sim].obs;
            let u = f.value + cfg.hp_coef * phi(obs) + cfg.pot_w * belts(obs);
            if self.node(need.node).child(need.action, need.key).is_none() {
                let id = self.add(Node::new(&f.logits, &f.prior, Some(u)));
                self.node_mut(need.node)
                    .children
                    .push((need.action, need.key, id));
            }
            self.sims[need.sim].util = u;
        }
        self.stats.leaf_evals += needs.len() as u64;
        self.backup();
    }

    /// Adds each simulation's utility along its path, in simulation order.
    fn backup(&mut self) {
        for j in 0..self.wave {
            let util = self.sims[j].util;
            for k in 0..self.sims[j].path.len() {
                let (id, a) = self.sims[j].path[k];
                let n = &mut self.nodes[id as usize];
                let slot = n.slot(a).expect("a legal move");
                n.w[slot] += util;
            }
            self.sims[j].path.clear();
        }
        self.wave = 0;
    }

    /// The move: the last candidate standing (the best by score if several), and the improved
    /// policy `softmax(logits + sigma(completed Q))`.
    fn conclude(&mut self, cfg: &SearchConfig, width: usize) {
        let s = self.scores(cfg, width);
        self.pick = if self.cands.len() == 1 {
            self.cands[0]
        } else {
            let mut best = self.cands[0];
            for &a in &self.cands[1..] {
                if s[usize::from(a)] > s[usize::from(best)] {
                    best = a;
                }
            }
            best
        };
        let mut scratch = std::mem::take(&mut self.scratch);
        let root = self.node(self.root);
        let cq = root.completed_q(width, &mut scratch);
        let scale = (cfg.c_visit + root.max_visits()) * cfg.c_scale;
        let z: Vec<f64> = (0..root.acts.len())
            .map(|k| root.logits[k] + scale * cq[k])
            .collect();
        let zmax = z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let e: Vec<f64> = z.iter().map(|&x| (x - zmax).exp()).collect();
        let sum: f64 = e.iter().sum();
        let mut t: Vec<(u16, f64)> = root
            .acts
            .iter()
            .zip(&e)
            .map(|(&a, &x)| (a, x / sum))
            .collect();
        t.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.visits = root.visits(width, &mut scratch);
        self.target = t;
        self.scratch = scratch;
        self.played = Some((self.root, self.pick));
        self.active = false;
        self.done = true;
    }
}

/// The search over many fights at once (module docs).
#[derive(Debug)]
pub struct Search {
    cfg: SearchConfig,
    run_config: RunConfig,
    /// By main env index.
    fights: Vec<Option<Fight>>,
    /// The fights of the current decision, in row order.
    rows: Vec<usize>,
    /// The fights whose leaves the last [`Search::run`] requested, with their counts, in order.
    requested: Vec<(usize, usize)>,
    width: usize,
    stats: SearchStats,
}

/// A fight's decision ([`Search::results`]).
#[derive(Clone, Copy, Debug)]
pub struct SearchResult<'a> {
    /// The move.
    pub action: u16,
    /// The root's visits.
    pub visits: f64,
    /// The improved policy (action, probability), most probable first.
    pub policy: &'a [(u16, f64)],
}

/// A request row's destination buffers ([`Search::run`]).
#[derive(Debug)]
pub struct Requests<'a> {
    pub obs: &'a mut [u8],
    pub masks: &'a mut [u64],
    pub privs: &'a mut [u8],
}

impl Search {
    /// A search over the envs of `envs` (their configuration builds the copies).
    #[must_use]
    pub fn new(cfg: SearchConfig, envs: &RunEnvs) -> Self {
        Self {
            cfg,
            run_config: envs.config,
            fights: Vec::new(),
            rows: Vec::new(),
            requested: Vec::new(),
            width: RUN_ACTIONS,
            stats: SearchStats::default(),
        }
    }

    /// The counters, summed over the fights.
    #[must_use]
    pub fn stats(&self) -> SearchStats {
        let mut s = self.stats;
        for f in self.fights.iter().flatten() {
            s.add(&f.stats);
        }
        s
    }

    /// Starts a decision for main envs `mains` (row `r` searches `mains[r]`): `logits` and
    /// `prior` (`width` per row, `-inf` / 0 at illegal actions), `gumbel` (`width` per row, or
    /// empty for none), `seeds` (`per_row` world seeds per row: paired or one world; for fresh
    /// worlds the first seeds a generator).
    ///
    /// # Panics
    /// If a slice has the wrong length or a main env is out of range.
    #[allow(clippy::too_many_arguments, reason = "the FFI's flat arrays")]
    pub fn begin(
        &mut self,
        envs: &RunEnvs,
        mains: &[usize],
        width: usize,
        logits: &[f64],
        prior: &[f64],
        gumbel: &[f64],
        seeds: &[u64],
        per_row: usize,
    ) {
        assert_eq!(logits.len(), mains.len() * width, "logits per row");
        assert_eq!(prior.len(), logits.len(), "priors per row");
        assert!(
            gumbel.is_empty() || gumbel.len() == logits.len(),
            "gumbel per row"
        );
        assert_eq!(seeds.len(), mains.len() * per_row, "seeds per row");
        assert!(per_row > 0, "at least one seed per row");
        self.width = width;
        self.rows.clear();
        self.requested.clear();
        for (r, &main) in mains.iter().enumerate() {
            if self.fights.len() <= main {
                self.fights.resize_with(main + 1, || None);
            }
            let config = self.run_config;
            let fight = self.fights[main].get_or_insert_with(|| Fight::new(config));
            let lg = &logits[r * width..][..width];
            let pr = &prior[r * width..][..width];
            let key = key_of(
                &envs.obs[main * RUN_OBS_BYTES..][..RUN_OBS_BYTES],
                &envs.masks[main * RUN_MASK_WORDS..][..RUN_MASK_WORDS],
            );
            let kept = fight
                .kept
                .take()
                .filter(|&(_, k)| k == key && self.cfg.keep_tree);
            let same = kept.is_some_and(|(id, _)| {
                let acts = &fight.node(id).acts;
                acts.len() == lg.iter().filter(|l| l.is_finite()).count()
                    && acts.iter().all(|&a| lg[usize::from(a)].is_finite())
            });
            if let (Some((id, _)), true) = (kept, same) {
                fight.root = fight.compact(id);
                fight.node_mut(0).set_logits(lg, pr);
                fight.stats.reused += 1;
            } else {
                fight.nodes.clear();
                let root = Node::new(lg, pr, None);
                fight.root = fight.add(root);
            }
            fight.source.copy_from(&envs.batch[main], None);
            fight.gumbel = if gumbel.is_empty() {
                vec![0.0; width]
            } else {
                gumbel[r * width..][..width].to_vec()
            };
            fight.seeds = seeds[r * per_row..][..per_row].to_vec();
            fight.rng = fight.seeds[0];
            fight.wcnt = vec![0; width];
            let root = fight.node(fight.root);
            let k = self.cfg.m.min(root.acts.len());
            let mut c: Vec<(u16, f64)> = root
                .acts
                .iter()
                .zip(&root.logits)
                .map(|(&a, &l)| (a, fight.gumbel[usize::from(a)] + l))
                .collect();
            // stable: equal scores keep the action order (`cb1_mcts` sorts with kind="stable")
            c.sort_by(|a, b| b.1.total_cmp(&a.1));
            fight.cands = c.iter().take(k).map(|&(a, _)| a).collect();
            fight.phases = if fight.cands.len() > 1 {
                usize::try_from(fight.cands.len().next_power_of_two().trailing_zeros())
                    .unwrap_or(1)
                    .max(1)
            } else {
                1
            };
            (fight.phase, fight.used, fight.in_phase) = (0, 0, false);
            fight.todo.clear();
            fight.needs.clear();
            fight.fed.clear();
            (fight.active, fight.done, fight.played) = (true, false, None);
            fight.stats.searched += 1;
            self.rows.push(main);
        }
    }

    /// Advances every fight to its next leaves and writes them (observation, mask, privileged
    /// buffer) into `out`, fight by fight in row order; the number of rows, 0 once every fight
    /// has its move.
    ///
    /// # Panics
    /// If `out` holds fewer rows than the round needs (at most `wave` per fight).
    pub fn run(&mut self, out: &mut Requests<'_>) -> usize {
        let cfg = self.cfg;
        let width = self.width;
        let work: Vec<&mut Fight> = self
            .fights
            .iter_mut()
            .flatten()
            .filter(|f| f.active)
            .collect();
        let cursor = AtomicUsize::new(0);
        let threads = cfg.threads.max(1).min(work.len().max(1));
        let cells: Vec<std::sync::Mutex<&mut Fight>> =
            work.into_iter().map(std::sync::Mutex::new).collect();
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let i = cursor.fetch_add(1, Ordering::Relaxed);
                        let Some(cell) = cells.get(i) else { break };
                        let mut f = cell.lock().expect("an unpoisoned fight");
                        f.advance(&cfg, width);
                    }
                });
            }
        });
        drop(cells);
        self.stats.rounds += 1;
        self.requested.clear();
        let mut row = 0;
        for &main in &self.rows {
            let f = self.fights[main].as_ref().expect("a fight");
            if f.needs.is_empty() {
                continue;
            }
            for need in &f.needs {
                let sim = &f.sims[need.sim];
                out.obs[row * RUN_OBS_BYTES..][..RUN_OBS_BYTES].copy_from_slice(&sim.obs);
                out.masks[row * RUN_MASK_WORDS..][..RUN_MASK_WORDS].copy_from_slice(&sim.mask);
                out.privs[row * PRIV_BYTES..][..PRIV_BYTES].copy_from_slice(&sim.privs);
                row += 1;
            }
            self.requested.push((main, f.needs.len()));
        }
        row
    }

    /// The net's outputs for the last [`Search::run`]'s rows: `logits` and `prior` (`width`
    /// per row), `values`.
    ///
    /// # Panics
    /// If the lengths do not match the request.
    pub fn feed(&mut self, logits: &[f64], prior: &[f64], values: &[f64]) {
        let w = self.width;
        let total: usize = self.requested.iter().map(|&(_, n)| n).sum();
        assert_eq!(values.len(), total, "one value per requested row");
        assert_eq!(logits.len(), total * w, "logits per row");
        assert_eq!(prior.len(), total * w, "priors per row");
        let mut row = 0;
        for &(main, n) in &self.requested {
            let f = self.fights[main].as_mut().expect("a fight");
            f.fed = (row..row + n)
                .map(|r| Fed {
                    logits: logits[r * w..][..w].to_vec(),
                    prior: prior[r * w..][..w].to_vec(),
                    value: values[r],
                })
                .collect();
            row += n;
        }
        self.requested.clear();
    }

    /// Per row of the decision: the move, the root's visits and the improved policy.
    ///
    /// # Panics
    /// If called before [`Search::begin`].
    #[must_use]
    pub fn results(&self) -> Vec<SearchResult<'_>> {
        self.rows
            .iter()
            .map(|&main| {
                let f = self.fights[main].as_ref().expect("a fight");
                SearchResult {
                    action: f.pick,
                    visits: f.visits,
                    policy: &f.target,
                }
            })
            .collect()
    }

    /// After the real step of main envs `mains` (`cb1_mcts.MCTSPlayer.after_step`): each
    /// searched fight keeps the child of its move whose key is the real position's, if visited.
    pub fn after_step(&mut self, envs: &RunEnvs, mains: &[usize]) {
        let width = self.width;
        for &main in mains {
            let Some(Some(f)) = self.fights.get_mut(main) else {
                continue;
            };
            f.kept = None;
            let Some((node, a)) = f.played.take() else {
                continue;
            };
            if envs.done[main] != 0 {
                continue;
            }
            let key = key_of(
                &envs.obs[main * RUN_OBS_BYTES..][..RUN_OBS_BYTES],
                &envs.masks[main * RUN_MASK_WORDS..][..RUN_MASK_WORDS],
            );
            if let Some(c) = f.node(node).child(a, key) {
                let mut scratch = std::mem::take(&mut f.scratch);
                if f.node(c).visits(width, &mut scratch) > 0.0 {
                    f.kept = Some((c, key));
                }
                f.scratch = scratch;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::np_sum;

    #[test]
    fn np_sum_blocks() {
        let a: Vec<f64> = (0..300).map(|i| f64::from(i) * 0.1).collect();
        for n in [0, 1, 7, 8, 9, 64, 128, 129, 300] {
            let s = np_sum(&a[..n]);
            let exact: f64 = (0..n)
                .map(|i| f64::from(u32::try_from(i).unwrap_or(0)) * 0.1)
                .sum();
            assert!((s - exact).abs() < 1e-9, "{n}");
        }
    }
}
