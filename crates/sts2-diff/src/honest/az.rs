//! An `AlphaZero`-style combat search (RLB13): one PUCT tree over several turns from the visible
//! position, the value net's policy head as its prior and its value head at the leaves.
//!
//! Every simulation samples one world from the root ([`Combat::blindfold_for_search`]: the
//! unknown piles shuffled, the RNG streams reseeded) and walks the tree in it. Nodes are keyed by
//! what the player sees ([`tree::visible_key`]), so play orders that reach the same position share
//! a node (a turn's plays do not explode the tree) and a decision never depends on a world's
//! hidden part. An edge is a chance node: its outcomes are the distinct visible positions the
//! action led to (a draw, an enemy's roll, the next turn's hand). Outcomes widen progressively;
//! past the limit a simulation is folded into an existing outcome, sampled by how often it came
//! up, by replaying that outcome's world (its seed and its path's action keys) from the root.
//! Nodes hold statistics and edges only, no `Combat`.
//!
//! A new node is valued by the net: P(win) + w × the HP fraction kept (capped at the HP the
//! player has), on one scale with a won fight (1 + a certainty bonus + w × its HP fraction) and a
//! lost one (0). A potion costs a little on its edge. The world seeds are hashes of the
//! observation, so the search reads no hidden state.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    reason = "the search's statistics (means, PUCT, the net's probabilities); they decide no game rule"
)]

use std::collections::HashMap;

use sts2_core::Combat;
use sts2_core::combat::{Action, CombatResult};
use sts2_rl::obs::{OBS_BYTES, Observer};

use super::search::{self, FightContext, Scope, answer_choice, resolve_choices};
use super::tree::{END_TURN_KEY, action_key, visible_key};
use super::value::{Output, ValueNet};

/// The most card plays one turn holds in the tree.
const MAX_PLAYS: u8 = 12;
/// The deepest a simulation walks (a guard).
const MAX_DEPTH: usize = 120;
/// A won fight's edge over a position the net values the same.
const WON_BONUS: f64 = 0.005;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// How hard the tree looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AzConfig {
    /// Simulations per decision in a common, elite and boss fight.
    pub sims: u32,
    pub big_sims: u32,
    pub boss_sims: u32,
    /// The PUCT constant (values in fight wins).
    pub c: f64,
    /// Progressive widening: an edge holds at most `ceil(k · (n + 1)^alpha)` outcomes; an
    /// end-turn edge `end_k` instead of `k`.
    pub widen_k: f64,
    pub end_k: f64,
    pub widen_alpha: f64,
    /// Turn ends the tree spans: past the last, the position is a leaf valued by the net.
    pub turns: u8,
    /// The HP fraction kept, in thousandths of a win.
    pub hp: i64,
    /// A potion's cost, in millionths of a win.
    pub potion: i64,
    /// The prior: the policy at a temperature, mixed with uniform.
    pub prior_temp: f32,
    pub prior_mix: f64,
    /// The fights the tree plays; the others are played by the value-leaf beam search.
    pub scope: Scope,
    /// Simulations descended before their leaves are valued in one batch of the net (each
    /// path holds a virtual loss meanwhile).
    pub batch: u32,
    /// With it non-zero, the prior is a softmax of each action's turn score (the beam's: the
    /// turn ended after it) at this temperature in points, instead of the policy head.
    pub turn_prior: f64,
}

impl Default for AzConfig {
    fn default() -> Self {
        Self {
            sims: 200,
            big_sims: 800,
            boss_sims: 4000,
            // RLB13 screens: c 0.3 and a uniform prior (the policy head's prior lost 2 HP an
            // elite fight); the tree in boss fights only (elites tie the beam at 2.5x the time).
            c: 0.3,
            widen_k: 1.0,
            end_k: 2.0,
            widen_alpha: 0.5,
            turns: 4,
            hp: 300,
            potion: 5_000,
            prior_temp: 1.0,
            prior_mix: 1.0,
            scope: Scope::Boss,
            batch: 1,
            turn_prior: 0.0,
        }
    }
}

impl AzConfig {
    /// The budget for a fight.
    #[must_use]
    pub const fn sims_for(&self, context: FightContext) -> u32 {
        if context.boss {
            self.boss_sims
        } else if context.big {
            self.big_sims
        } else {
            self.sims
        }
    }
}

/// One visible position an edge led to, with a world that reaches it: the root's world `seed`
/// and the action keys from the root.
#[derive(Debug)]
struct Outcome {
    key: u64,
    child: usize,
    hits: u32,
    seed: u64,
    path: Box<[u64]>,
}

#[derive(Debug)]
struct Edge {
    key: u64,
    /// The action in the position the node was expanded in (ids may differ in another world).
    action: Action,
    prior: f64,
    /// A potion's cost (negative), added to every value backed up through the edge.
    cost: f64,
    visits: u32,
    total: f64,
    outcomes: Vec<Outcome>,
}

#[derive(Debug)]
struct Node {
    plays: u8,
    potion: bool,
    /// Turn ends from the root.
    turns: u8,
    edges: Vec<Edge>,
    visits: u32,
    total: f64,
    /// The net's value of the position (or a won or lost fight's).
    value: f64,
    terminal: bool,
}

struct Tree<'a> {
    nodes: Vec<Node>,
    by_key: HashMap<u64, usize>,
    observer: Observer,
    obs: Vec<u8>,
    net: &'a ValueNet,
    config: AzConfig,
    weights: search::Weights,
    potions_ok: bool,
    /// The fold sampler's state.
    rng: u64,
}

/// The tree's action among `legal` (the fight's actions the run accepts, non-empty).
#[must_use]
pub fn choose(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: AzConfig,
    net: &ValueNet,
) -> Action {
    best(&search_root(combat, legal, context, config, net)).unwrap_or_else(|| legal[0].clone())
}

/// The most visited of `root`'s actions (by mean value among equals).
#[must_use]
pub fn best(root: &[(Action, u32, f64)]) -> Option<Action> {
    root.iter()
        .max_by(|a, b| {
            (a.1, a.2)
                .partial_cmp(&(b.1, b.2))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(a, _, _)| a.clone())
}

/// The root's actions after the search: each with its visits and mean value. A forced or
/// answered choice is the only action, with one visit.
#[must_use]
pub fn search_root(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: AzConfig,
    net: &ValueNet,
) -> Vec<(Action, u32, f64)> {
    if combat.pending_decision().is_some()
        && let Some(answer) = answer_choice(combat)
    {
        return vec![(answer, 1, 0.0)];
    }
    let first: Vec<Action> = legal
        .iter()
        .filter(|a| !matches!(a, Action::Resolve(_)))
        .cloned()
        .collect();
    if first.len() <= 1 {
        let only = first.into_iter().next().unwrap_or_else(|| legal[0].clone());
        return vec![(only, 1, 0.0)];
    }
    let before = search::player_hp(combat.state());
    let base = search::world_seed(combat);
    let mut tree = Tree {
        nodes: Vec::new(),
        by_key: HashMap::new(),
        observer: Observer::new(true),
        obs: vec![0; OBS_BYTES],
        net,
        config,
        weights: search::Weights::of(context),
        potions_ok: context.big || before.0 * 100 < before.1 * 60,
        rng: base | 1,
    };
    let root_world = combat.blindfold_for_search(base);
    let key = tree.key(&root_world, 0, false);
    let out = net.eval(&root_world);
    let root = tree.add(&root_world, 0, false, 0, key, Some(&first), Some(&out));
    let sims = config.sims_for(context);
    let batch = config.batch.max(1);
    let mut sim = 0;
    while sim < sims {
        let pending: Vec<Pending> = (sim..sims.min(sim + batch))
            .map(|k| tree.descend(combat, root, base ^ u64::from(k + 1).wrapping_mul(GOLDEN)))
            .collect();
        sim += batch;
        tree.finish(pending);
    }
    tree.nodes[root]
        .edges
        .iter()
        .map(|e| (e.action.clone(), e.visits, mean(e)))
        .collect()
}

fn mean(e: &Edge) -> f64 {
    if e.visits == 0 {
        f64::NEG_INFINITY
    } else {
        e.total / f64::from(e.visits)
    }
}

impl Tree<'_> {
    fn key(&mut self, combat: &Combat, plays: u8, potion: bool) -> u64 {
        visible_key(&mut self.observer, &mut self.obs, combat, plays, potion)
    }

    /// The node with `key` (for `state`), added if it is new, valued by `out` (the net on
    /// `state`; `None` only for an ended fight). `legal` overrides the position's legal actions
    /// (the root's).
    #[allow(
        clippy::too_many_arguments,
        reason = "a node's turn state, its key and its net reading"
    )]
    fn add(
        &mut self,
        state: &Combat,
        plays: u8,
        potion: bool,
        turns: u8,
        key: u64,
        legal: Option<&[Action]>,
        out: Option<&Output>,
    ) -> usize {
        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }
        let s = state.state();
        let (hp, max) = search::player_hp(s);
        let hp_w = self.config.hp as f64 / 1000.0;
        let frac = f64::from(hp.max(0)) / f64::from(max.max(1));
        let terminal = terminal_value(state, hp_w);
        let (value, edges) = match (terminal, out) {
            (Some(v), _) => (v, Vec::new()),
            (None, Some(out)) => self.expand(state, plays, potion, legal, hp_w, frac, out),
            (None, None) => (0.0, Vec::new()),
        };
        let id = self.nodes.len();
        self.nodes.push(Node {
            plays,
            potion,
            turns,
            edges,
            visits: 0,
            total: 0.0,
            value,
            terminal: terminal.is_some(),
        });
        self.by_key.insert(key, id);
        id
    }

    /// The value of `state` by `out` (the net on it), and its edges with their priors.
    #[allow(
        clippy::too_many_arguments,
        reason = "a node's turn state, its HP terms and its net reading"
    )]
    fn expand(
        &mut self,
        state: &Combat,
        plays: u8,
        potion: bool,
        legal: Option<&[Action]>,
        hp_w: f64,
        frac: f64,
        out: &Output,
    ) -> (f64, Vec<Edge>) {
        let actions = legal.map_or_else(|| state.legal_actions(), <[Action]>::to_vec);
        self.observer.observe(state, &mut self.obs);
        let mut edges: Vec<Edge> = Vec::new();
        for action in actions {
            let cost = match &action {
                Action::Resolve(_) => continue,
                Action::PlayCard { .. } if plays >= MAX_PLAYS => continue,
                Action::UsePotion { .. } if !self.potions_ok || potion => continue,
                Action::UsePotion { .. } => -(self.config.potion as f64) / 1e6,
                _ => 0.0,
            };
            let key = action_key(&self.obs, state, &action);
            if edges.iter().any(|e| e.key == key) {
                continue;
            }
            edges.push(Edge {
                key,
                action,
                prior: 0.0,
                cost,
                visits: 0,
                total: 0.0,
                outcomes: Vec::new(),
            });
        }
        let value = f64::from(out.win) + hp_w * f64::from(out.hp).min(frac);
        let n = edges.len().max(1) as f64;
        let mix = self.config.prior_mix;
        if mix >= 1.0 {
            for e in &mut edges {
                e.prior = 1.0 / n;
            }
            return (value, edges);
        }
        let actions: Vec<Action> = edges.iter().map(|e| e.action.clone()).collect();
        let p = if self.config.turn_prior > 0.0 {
            self.turn_priors(state, &actions, potion)
        } else {
            ValueNet::priors(out, state, &actions, self.config.prior_temp)
        };
        for (e, &q) in edges.iter_mut().zip(&p) {
            e.prior = (1.0 - mix) * f64::from(q) + mix / n;
        }
        (value, edges)
    }

    /// A softmax of each of `actions`' turn score in `state` (0 for one that loses).
    fn turn_priors(&self, state: &Combat, actions: &[Action], potion: bool) -> Vec<f32> {
        let scores: Vec<Option<i64>> = actions
            .iter()
            .map(|a| match a {
                Action::EndTurn => Some(search::eval(
                    &search::end_turn_of(state.clone()),
                    self.weights,
                )),
                _ => search::expand(state, a, true, potion, self.weights).map(|(s, _, _)| s),
            })
            .collect();
        let top = scores.iter().flatten().copied().max().unwrap_or(0);
        let exp: Vec<f64> = scores
            .iter()
            .map(|s| {
                s.map_or(0.0, |s| {
                    ((s.saturating_sub(top)) as f64 / self.config.turn_prior).exp()
                })
            })
            .collect();
        let sum: f64 = exp.iter().sum::<f64>().max(f64::MIN_POSITIVE);
        exp.iter().map(|e| (e / sum) as f32).collect()
    }

    /// The value of a live position `world` by `out` (the net on it).
    fn net_value(&self, world: &Combat, out: &Output) -> f64 {
        let (hp, max) = search::player_hp(world.state());
        let hp_w = self.config.hp as f64 / 1000.0;
        let frac = f64::from(hp.max(0)) / f64::from(max.max(1));
        f64::from(out.win) + hp_w * f64::from(out.hp).min(frac)
    }

    /// This world's action with edge key `key`.
    fn own_action(&mut self, world: &Combat, key: u64) -> Option<Action> {
        if key == END_TURN_KEY {
            return Some(Action::EndTurn);
        }
        let legal = world.legal_actions();
        self.observer.observe(world, &mut self.obs);
        legal
            .into_iter()
            .find(|a| action_key(&self.obs, world, a) == key)
    }

    /// `world` with `key` played (choices resolved after it); `None` if it does not apply.
    fn step(&mut self, mut world: Combat, key: u64, fallback: &Action) -> Option<Combat> {
        let action = self
            .own_action(&world, key)
            .unwrap_or_else(|| fallback.clone());
        world.apply(action).ok()?;
        Some(resolve_choices(world))
    }

    /// The root's world `seed` with the actions of `path` played.
    fn replay(&mut self, root: &Combat, seed: u64, path: &[u64]) -> Combat {
        let mut world = root.blindfold_for_search(seed);
        for &key in path {
            // The path was played in this world before, so every step applies.
            world = match self.step(world, key, &Action::EndTurn) {
                Some(next) => next,
                None => return root.blindfold_for_search(seed),
            };
        }
        world
    }

    /// The next fold sample in `0..n`.
    fn sample(&mut self, n: u64) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng % n.max(1)
    }

    /// One simulation's descent from `root` in the root's world `seed`, to a leaf; its path
    /// holds a virtual loss (a visit worth 0) until [`Tree::finish`].
    #[expect(
        clippy::too_many_lines,
        reason = "one PUCT descent: selection, chance outcomes and widening read best in one place"
    )]
    fn descend(&mut self, combat: &Combat, root: usize, seed: u64) -> Pending {
        let mut seed = seed;
        let mut world = combat.blindfold_for_search(seed);
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut keys: Vec<u64> = Vec::new();
        let mut id = root;
        let leaf = loop {
            let node = &self.nodes[id];
            if node.terminal || node.edges.is_empty() || path.len() >= MAX_DEPTH {
                break Leaf::Done(node.value);
            }
            let e = self.select(id);
            path.push((id, e));
            let node = &self.nodes[id];
            let edge = &node.edges[e];
            let (ekey, fallback, n) = (edge.key, edge.action.clone(), edge.visits);
            keys.push(ekey);
            let end = ekey == END_TURN_KEY;
            let potion = matches!(fallback, Action::UsePotion { .. });
            let (plays, potion, turns) = if end {
                (0, false, node.turns + 1)
            } else if potion {
                (node.plays, true, node.turns)
            } else {
                (node.plays + 1, node.potion, node.turns)
            };
            world = match self.step(world, ekey, &fallback) {
                Some(next) => next,
                None => break Leaf::Done(0.0),
            };
            let hp_w = self.config.hp as f64 / 1000.0;
            if end && turns >= self.config.turns {
                break terminal_value(&world, hp_w)
                    .map_or_else(|| Leaf::Horizon(Box::new(world)), Leaf::Done);
            }
            let outcome = self.key(&world, plays, potion);
            let edge = &mut self.nodes[id].edges[e];
            if let Some(o) = edge.outcomes.iter_mut().find(|o| o.key == outcome) {
                o.hits += 1;
                id = o.child;
                continue;
            }
            let k = if end {
                self.config.end_k
            } else {
                self.config.widen_k
            };
            let limit = (k * f64::from(n + 1).powf(self.config.widen_alpha))
                .ceil()
                .max(1.0);
            if (edge.outcomes.len() as f64) < limit {
                let found = self.by_key.get(&outcome).copied();
                let ended = terminal_value(&world, hp_w).is_some();
                if found.is_some() || ended {
                    let child = found.unwrap_or_else(|| {
                        self.add(&world, plays, potion, turns, outcome, None, None)
                    });
                    self.nodes[id].edges[e].outcomes.push(Outcome {
                        key: outcome,
                        child,
                        hits: 1,
                        seed,
                        path: keys.clone().into_boxed_slice(),
                    });
                    if ended {
                        break Leaf::Done(self.nodes[child].value);
                    }
                    id = child;
                    continue;
                }
                break Leaf::Expand(Box::new(Expansion {
                    world,
                    at: (id, e),
                    key: outcome,
                    plays,
                    potion,
                    turns,
                    seed,
                    path: keys.into_boxed_slice(),
                }));
            }
            // Past the limit: an existing outcome, sampled by how often it came up, in a world
            // that reaches it.
            let total: u64 = edge.outcomes.iter().map(|o| u64::from(o.hits)).sum();
            let mut pick = self.sample(total);
            let edge = &self.nodes[id].edges[e];
            let o = edge
                .outcomes
                .iter()
                .find(|o| {
                    let hit = pick < u64::from(o.hits);
                    pick = pick.saturating_sub(u64::from(o.hits));
                    hit
                })
                .unwrap_or(&edge.outcomes[0]);
            let (child, o_seed, o_path) = (o.child, o.seed, o.path.clone());
            seed = o_seed;
            world = self.replay(combat, seed, &o_path);
            keys = o_path.into_vec();
            id = child;
        };
        for &(id, e) in &path {
            let node = &mut self.nodes[id];
            node.visits += 1;
            node.edges[e].visits += 1;
        }
        Pending { path, leaf }
    }

    /// Values `pending`'s leaves (the net in one batch), adds their new nodes and backs the
    /// values up.
    fn finish(&mut self, pending: Vec<Pending>) {
        let worlds: Vec<&Combat> = pending
            .iter()
            .filter_map(|p| match &p.leaf {
                Leaf::Done(_) => None,
                Leaf::Horizon(world) => Some(&**world),
                Leaf::Expand(x) => Some(&x.world),
            })
            .collect();
        let outs = self.net.eval_batch(&worlds);
        drop(worlds);
        let mut outs = outs.into_iter();
        for p in pending {
            let value = match p.leaf {
                Leaf::Done(v) => v,
                Leaf::Horizon(world) => {
                    let out = outs.next().expect("an output per leaf");
                    self.net_value(&world, &out)
                }
                Leaf::Expand(x) => {
                    let out = outs.next().expect("an output per leaf");
                    let x = *x;
                    let child = self.add(
                        &x.world,
                        x.plays,
                        x.potion,
                        x.turns,
                        x.key,
                        None,
                        Some(&out),
                    );
                    let (id, e) = x.at;
                    let edge = &mut self.nodes[id].edges[e];
                    if !edge.outcomes.iter().any(|o| o.key == x.key) {
                        edge.outcomes.push(Outcome {
                            key: x.key,
                            child,
                            hits: 1,
                            seed: x.seed,
                            path: x.path,
                        });
                    }
                    self.nodes[child].value
                }
            };
            self.backup(&p.path, value);
        }
    }

    /// The PUCT edge of node `id` (which has edges).
    fn select(&self, id: usize) -> usize {
        let node = &self.nodes[id];
        let parent = if node.visits > 0 {
            node.total / f64::from(node.visits)
        } else {
            node.value
        };
        let sqrt_n = f64::from(node.visits.max(1)).sqrt();
        let mut best = 0;
        let mut best_score = f64::NEG_INFINITY;
        for (i, e) in node.edges.iter().enumerate() {
            let q = if e.visits == 0 {
                parent
            } else {
                e.total / f64::from(e.visits)
            };
            let score = q + self.config.c * e.prior * sqrt_n / (1.0 + f64::from(e.visits));
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        best
    }

    /// Backs `value` up `path` (whose visits the descent counted), each edge adding its cost.
    fn backup(&mut self, path: &[(usize, usize)], value: f64) {
        let mut v = value;
        for &(id, e) in path.iter().rev() {
            let node = &mut self.nodes[id];
            let edge = &mut node.edges[e];
            v += edge.cost;
            edge.total += v;
            node.total += v;
        }
    }
}

/// A descended simulation: its path and its leaf.
struct Pending {
    path: Vec<(usize, usize)>,
    leaf: Leaf,
}

enum Leaf {
    /// Valued already (an ended fight, a known node).
    Done(f64),
    /// A position past the horizon, valued by the net.
    Horizon(Box<Combat>),
    /// A new outcome of an edge: a node to add.
    Expand(Box<Expansion>),
}

/// A new outcome of edge `at`, reached in `world` (the root's world `seed` with `path` played).
struct Expansion {
    world: Combat,
    at: (usize, usize),
    key: u64,
    plays: u8,
    potion: bool,
    turns: u8,
    seed: u64,
    path: Box<[u64]>,
}

/// The value of an ended fight in `world` (`None` while it goes on).
fn terminal_value(world: &Combat, hp_w: f64) -> Option<f64> {
    let s = world.state();
    let (hp, max) = search::player_hp(s);
    let frac = f64::from(hp.max(0)) / f64::from(max.max(1));
    match s.result {
        Some(CombatResult::Won) if hp > 0 => Some(1.0 + WON_BONUS + hp_w * frac),
        Some(_) => Some(0.0),
        None if hp <= 0 => Some(0.0),
        None => None,
    }
}
