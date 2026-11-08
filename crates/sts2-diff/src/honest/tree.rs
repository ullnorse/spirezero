//! An honest single-tree combat search with chance nodes (RLB7).
//!
//! The root is the visible position. Every simulation samples one world from it
//! ([`Combat::blindfold_for_search`]: the unknown piles in card order, then shuffled, and the RNG
//! streams reseeded) and walks the tree in that world. A decision node picks an edge by PUCT; the
//! edge is a chance node whose outcomes are the distinct *visible* positions the action leads to
//! (a draw from the unknown pile, an enemy's roll, the next turn's hand). Outcomes widen
//! progressively: once an edge has `ceil(k · n^α)` of them, a new outcome is folded into an
//! existing one, sampled by visits, whose stored position is blindfolded again with a fresh seed
//! (its hidden part resampled). Positions are keyed by what the player sees with identical cards
//! made interchangeable, so play orders that reach the same position share a node and two copies
//! of a card are one edge.
//!
//! A new node is valued as RLB6 valued a plan's end: the turn score (`eval`) at the next player
//! turn, plus a full-fight playout in elite and boss fights; a power played or a potion used on
//! the path adds RLB6's bonus or cost. Every seed is a hash of the observation, so the search
//! reads no hidden state.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    reason = "the search's statistics (means, PUCT); they decide no game rule: small counts and HP become floats, and exactness does not matter"
)]

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use sts2_core::Combat;
use sts2_core::combat::{Action, CombatResult};
use sts2_core::content::cards::CardType;
use sts2_rl::obs::{
    CARD_BYTES, CARDS, CARDS_AT, CHOICE_AT, CREATURES_AT, INTENTS_AT, OBS_BYTES, Observer,
};

use super::rollout::{RolloutNet, Scratch};
use super::search::{
    self, FightContext, POWER_BONUS, Playout, Weights, answer_choice, end_turn_of, eval,
    playout_value, resolve_choices,
};

/// The most card plays one turn holds in the tree.
const MAX_PLAYS: u8 = 12;
/// The value of a won fight: above any position's.
const WIN_VALUE: f64 = 50_000.0;
/// The value of a lost fight: below any position's.
const LOSS_VALUE: f64 = -50_000.0;
/// The deepest a simulation walks (a guard; turns end long before).
const MAX_DEPTH: usize = 200;

/// How hard the tree looks.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches of the search, each its own A/B option"
)]
pub struct TreeConfig {
    /// Simulations per decision in a common fight.
    pub sims: u32,
    /// Simulations per decision in an elite fight.
    pub big_sims: u32,
    /// Simulations per decision in a boss fight.
    pub boss_sims: u32,
    /// The PUCT constant at 1,000 simulations (normalized values); it scales with
    /// `sqrt(sims / 1000)`, so a larger budget explores wider.
    pub c: f64,
    /// Progressive widening: an edge holds at most `ceil(k · n^alpha)` outcomes.
    pub widen_k: f64,
    pub widen_alpha: f64,
    /// Full-fight playouts at new nodes in elite and boss fights.
    pub playouts: bool,
    /// Playouts in common fights too.
    pub playouts_all: bool,
    /// The value span the PUCT constant is in units of, with and without playouts.
    pub scale_playout: f64,
    pub scale_turn: f64,
    /// PUCT priors from the learned policy (when it is loaded), at a temperature, mixed with
    /// uniform.
    pub prior: bool,
    pub prior_temp: f32,
    pub prior_mix: f64,
    /// Plays one at a time, or whole-turn plans.
    pub mode: Mode,
    /// Common fights are played by RLB6's determinized search (it beat the tree there).
    pub hybrid: bool,
    /// Turn ends the tree spans: past the last, the position is a leaf valued in each world.
    pub turns: u8,
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            sims: 400,
            big_sims: 1600,
            boss_sims: 1600,
            c: 1.0,
            widen_k: 1.0,
            widen_alpha: 0.5,
            playouts: true,
            playouts_all: false,
            turns: 1,
            mode: Mode::Atomic,
            hybrid: true,
            prior: true,
            prior_temp: 1.0,
            prior_mix: 0.25,
            scale_playout: 30_000.0,
            scale_turn: 3_000.0,
        }
    }
}

impl TreeConfig {
    /// The budget for a fight.
    const fn sims_for(&self, context: FightContext) -> u32 {
        if context.boss {
            self.boss_sims
        } else if context.big {
            self.big_sims
        } else {
            self.sims
        }
    }
}

/// An edge: one action from a node, and the visible outcomes it led to.
#[derive(Debug)]
struct Edge {
    key: u64,
    /// The PUCT prior.
    prior: f64,
    /// The action in the node's stored position (its ids may differ in another world).
    action: Action,
    /// RLB6's plan terms for this action: a power's bonus, a potion's cost.
    bonus: f64,
    visits: u32,
    total: f64,
    outcomes: Vec<(u64, usize)>,
}

/// A decision node: a visible position.
#[derive(Debug)]
struct Node {
    /// Plays made this turn, and whether one was a potion (both outside the visible key).
    plays: u8,
    potion: bool,
    /// The position starts a player turn (it was reached by ending one).
    fresh: bool,
    /// Turn ends on the way from the root.
    turns: u8,
    /// The edges, once the node is expanded.
    edges: Option<Vec<Edge>>,
    visits: u32,
    total: f64,
    /// The value of a won or lost position.
    terminal: Option<f64>,
}

/// The search's working state for one decision.
struct Tree<'a> {
    nodes: Vec<Node>,
    by_key: HashMap<u64, usize>,
    observer: Observer,
    obs: Vec<u8>,
    weights: Weights,
    potions_ok: bool,
    playout: Option<Playout<'a>>,
    c: f64,
    config: TreeConfig,
    /// The value span PUCT's constant is in units of.
    scale: f64,
    /// The learned policy, for the PUCT priors.
    prior: Option<&'a RolloutNet>,
    scratch: Scratch,
}

/// The tree's action among `legal` (the fight's actions the run accepts, non-empty).
///
/// # Panics
/// Never: the root is expanded before the simulations.
#[must_use]
pub fn choose(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: TreeConfig,
    playout_policy: Playout<'_>,
) -> Action {
    if combat.pending_decision().is_some()
        && let Some(answer) = answer_choice(combat)
    {
        return answer;
    }
    let first: Vec<Action> = legal
        .iter()
        .filter(|a| !matches!(a, Action::Resolve(_)))
        .cloned()
        .collect();
    if first.len() <= 1 {
        return first.into_iter().next().unwrap_or_else(|| legal[0].clone());
    }
    let sims = config.sims_for(context);
    let before = search::player_hp(combat.state());
    let playouts = config.playouts && (context.big || config.playouts_all);
    let mut tree = Tree {
        nodes: Vec::new(),
        by_key: HashMap::new(),
        observer: Observer::new(true),
        obs: vec![0; OBS_BYTES],
        weights: Weights::of(context),
        potions_ok: context.big || before.0 * 100 < before.1 * 60,
        playout: playouts.then_some(playout_policy),
        c: config.c * (f64::from(sims) / 1000.0).sqrt().max(0.5),
        config,
        prior: match playout_policy {
            Playout::Learned(net) if config.prior => Some(net),
            _ => None,
        },
        scratch: Scratch::default(),
        scale: if playouts {
            config.scale_playout
        } else {
            config.scale_turn
        },
    };
    let base = search::world_seed(combat);
    let root_world = combat.blindfold_for_search(base);
    let root = tree.add(&root_world, 0, false, true, None);
    // The root's edges are the real legal actions (the hand is the same in every world).
    let edges = tree.edges_for(root, &root_world, Some(&first));
    tree.nodes[root].edges = Some(edges);
    for sim in 0..sims {
        let world = combat.blindfold_for_search(base ^ u64::from(sim + 1).wrapping_mul(GOLDEN));
        tree.simulate(root, world);
    }
    let edges = tree.nodes[root]
        .edges
        .as_ref()
        .expect("the root is expanded");
    let best = edges.iter().enumerate().fold(0, |best, (i, e)| {
        let b = &edges[best];
        if (e.visits, mean(e)) > (b.visits, mean(b)) {
            i
        } else {
            best
        }
    });
    edges[best].action.clone()
}

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// An edge's mean value.
fn mean(e: &Edge) -> f64 {
    if e.visits == 0 {
        f64::NEG_INFINITY
    } else {
        e.total / f64::from(e.visits)
    }
}

/// FNV-1a over `bytes`, from `h`.
fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(h, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3))
}

const FNV: u64 = 0xCBF2_9CE4_8422_2325;

/// The visible key of `combat` (with the plan's turn state): the observation with the card list
/// taken as a multiset and the hand's ids left out, plus the potion belt. `obs` is scratch.
pub(super) fn visible_key(
    observer: &mut Observer,
    obs: &mut [u8],
    combat: &Combat,
    plays: u8,
    potion: bool,
) -> u64 {
    if combat.state().result.is_some() {
        // An ended fight has no intents (reading a summoner's move would add a creature).
        Observer::new(false).observe(combat, obs);
        return fnv(FNV ^ 5, &obs[..CREATURES_AT]);
    }
    observer.observe(combat, obs);
    let mut h = fnv(FNV, &obs[..CARDS_AT]);
    let mut cards: Vec<[u8; CARD_BYTES]> = obs[CARDS_AT..CARDS_AT + CARDS * CARD_BYTES]
        .as_chunks::<CARD_BYTES>()
        .0
        .to_vec();
    cards.sort_unstable();
    for c in &cards {
        h = fnv(h, c);
    }
    // The intents; the hand's ids and a pending choice's options are card ids.
    h = fnv(h, &obs[INTENTS_AT..CHOICE_AT]);
    let mut hasher = DefaultHasher::new();
    for p in &combat.state().player.potions {
        p.map(|p| p.kind).hash(&mut hasher);
    }
    fnv(h ^ hasher.finish(), &[plays, u8::from(potion)])
}

/// The key of `action` in `combat`, whose observation `obs` holds: a card by its visible record
/// (two copies are one edge).
pub(super) fn action_key(obs: &[u8], combat: &Combat, action: &Action) -> u64 {
    match action {
        Action::PlayCard { card, target } => {
            let s = combat.state();
            let c = s.card(*card);
            let i = usize::try_from(card.0).unwrap_or(usize::MAX);
            let record = if i < CARDS {
                &obs[CARDS_AT + i * CARD_BYTES..CARDS_AT + (i + 1) * CARD_BYTES]
            } else {
                &[]
            };
            let mut hasher = DefaultHasher::new();
            (c.kind, c.upgrade_level).hash(&mut hasher);
            let h = fnv(FNV ^ 1 ^ hasher.finish(), record);
            fnv(h, &target.map_or(u32::MAX, |t| t.0).to_le_bytes())
        }
        Action::UsePotion { slot, target } => {
            let h = fnv(FNV ^ 2, &slot.to_le_bytes());
            fnv(h, &target.map_or(u32::MAX, |t| t.0).to_le_bytes())
        }
        Action::EndTurn => END_TURN_KEY,
        Action::Resolve(_) => FNV ^ 4,
    }
}

/// The key of [`Action::EndTurn`].
pub(super) const END_TURN_KEY: u64 = FNV ^ 3;

impl Tree<'_> {
    /// The visible key of `combat` (with the plan's turn state).
    fn key(&mut self, combat: &Combat, plays: u8, potion: bool) -> u64 {
        visible_key(&mut self.observer, &mut self.obs, combat, plays, potion)
    }

    /// The key of `action` in `combat` (whose observation `self.obs` holds).
    fn edge_key(&self, combat: &Combat, action: &Action) -> u64 {
        action_key(&self.obs, combat, action)
    }

    /// Adds a node for `state` (or finds the one with its key).
    fn add(
        &mut self,
        state: &Combat,
        plays: u8,
        potion: bool,
        fresh: bool,
        key: Option<u64>,
    ) -> usize {
        let key = key.unwrap_or_else(|| self.key(state, plays, potion));
        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }
        let s = state.state();
        let terminal = match s.result {
            Some(CombatResult::Won) if search::player_hp(s).0 > 0 => {
                Some(WIN_VALUE + 100.0 * f64::from(search::player_hp(s).0))
            }
            Some(_) => Some(LOSS_VALUE),
            None if search::player_hp(s).0 <= 0 => Some(LOSS_VALUE),
            None => None,
        };
        let id = self.nodes.len();
        self.nodes.push(Node {
            plays,
            potion,
            fresh,
            turns: 0,
            edges: None,
            visits: 0,
            total: 0.0,
            terminal,
        });
        self.by_key.insert(key, id);
        id
    }

    /// The edges of node `id`: `legal` (the root's) or its position's legal actions, one per
    /// visible key, within the plan's rules (a play cap, one potion a turn when potions pay).
    fn edges_for(&mut self, id: usize, state: &Combat, legal: Option<&[Action]>) -> Vec<Edge> {
        let node = &self.nodes[id];
        let (plays, potion) = (node.plays, node.potion);
        let actions = legal.map_or_else(|| state.legal_actions(), <[Action]>::to_vec);
        self.observer.observe(state, &mut self.obs);
        let s = state.state();
        let left: i32 = s
            .enemies
            .iter()
            .map(|&e| s.creature(e))
            .filter(|c| c.is_alive())
            .map(|c| c.hp + c.block)
            .sum();
        let mut edges: Vec<Edge> = Vec::new();
        for action in actions {
            let bonus = match &action {
                Action::Resolve(_) => continue,
                Action::EndTurn => 0.0,
                Action::PlayCard { .. } if plays >= MAX_PLAYS => continue,
                Action::UsePotion { .. } if !self.potions_ok || potion => continue,
                Action::UsePotion { .. } => -(self.weights.potion as f64),
                Action::PlayCard { card, .. } => {
                    if s.card(*card).kind.card_type() == CardType::Power {
                        (POWER_BONUS + 2 * i64::from(left)) as f64
                    } else {
                        0.0
                    }
                }
            };
            let key = self.edge_key(state, &action);
            if edges.iter().any(|e| e.key == key) {
                continue;
            }
            edges.push(Edge {
                key,
                prior: 0.0,
                action,
                bonus,
                visits: 0,
                total: 0.0,
                outcomes: Vec::new(),
            });
        }
        let n = edges.len().max(1) as f64;
        match self.prior {
            Some(net) => {
                let actions: Vec<Action> = edges.iter().map(|e| e.action.clone()).collect();
                let mut p = Vec::new();
                net.priors(
                    state,
                    &actions,
                    self.config.prior_temp,
                    &mut self.scratch,
                    &mut p,
                );
                let mix = self.config.prior_mix;
                for (e, &q) in edges.iter_mut().zip(&p) {
                    e.prior = (1.0 - mix) * f64::from(q) + mix / n;
                }
            }
            None => edges.iter_mut().for_each(|e| e.prior = 1.0 / n),
        }
        edges
    }

    /// One simulation from `root` in `world`.
    fn simulate(&mut self, root: usize, mut world: Combat) {
        // (node, edge) along the path, with each edge's bonus.
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut id = root;
        let value = loop {
            if let Some(v) = self.nodes[id].terminal {
                break v;
            }
            if path.len() >= MAX_DEPTH {
                break self.value(&world, self.nodes[id].fresh);
            }
            if self.nodes[id].edges.is_none() {
                let edges = self.edges_for(id, &world, None);
                self.nodes[id].edges = Some(edges);
            }
            let Some(e) = self.select(id) else {
                break self.value(&world, self.nodes[id].fresh);
            };
            path.push((id, e));
            let (key, action, n) = {
                let edge = &self.nodes[id].edges.as_ref().expect("expanded")[e];
                (edge.key, edge.action.clone(), edge.visits)
            };
            // This world's own action with the edge's key (a copy's id may differ).
            let action = self.own_action(&world, key).unwrap_or(action);
            let node = &self.nodes[id];
            let (plays, potion) = match action {
                Action::EndTurn => (0, false),
                Action::UsePotion { .. } => (node.plays, true),
                _ => (node.plays + 1, node.potion),
            };
            let end = matches!(action, Action::EndTurn);
            let turns = node.turns + u8::from(end);
            if world.apply(action).is_err() {
                break LOSS_VALUE;
            }
            world = resolve_choices(world);
            if turns >= self.config.turns {
                // The horizon: a chance leaf valued in this world.
                break self.value(&world, true);
            }
            let outcome = self.key(&world, plays, potion);
            let edge = &self.nodes[id].edges.as_ref().expect("expanded")[e];
            if let Some(&(_, child)) = edge.outcomes.iter().find(|(k, _)| *k == outcome) {
                id = child;
                continue;
            }
            let limit = (self.config.widen_k * f64::from(n + 1).powf(self.config.widen_alpha))
                .ceil()
                .max(1.0);
            if (edge.outcomes.len() as f64) < limit {
                let known = self.by_key.contains_key(&outcome);
                let child = self.add(&world, plays, potion, end, Some(outcome));
                self.nodes[child].turns = turns;
                self.nodes[id].edges.as_mut().expect("expanded")[e]
                    .outcomes
                    .push((outcome, child));
                if known {
                    id = child;
                    continue;
                }
                break self.leaf(child, &world);
            }
            // Past the widening limit a new outcome is not added: it is a leaf valued in this
            // world (no position is stored to fold it into).
            break self.value(&world, end);
        };
        self.backup(&path, value);
    }

    /// The value of new node `id` (whose position is `world`).
    fn leaf(&self, id: usize, world: &Combat) -> f64 {
        self.nodes[id]
            .terminal
            .unwrap_or_else(|| self.value(world, self.nodes[id].fresh))
    }

    /// RLB6's value of a position: the turn score at the next player turn (ending this one
    /// here unless it is just starting), plus the playout from there.
    fn value(&self, world: &Combat, fresh: bool) -> f64 {
        let ended = if fresh {
            world.clone()
        } else {
            end_turn_of(world.clone())
        };
        let s = ended.state();
        match s.result {
            Some(CombatResult::Won) if search::player_hp(s).0 > 0 => {
                return WIN_VALUE + 100.0 * f64::from(search::player_hp(s).0);
            }
            Some(_) => return LOSS_VALUE,
            None if search::player_hp(s).0 <= 0 => return LOSS_VALUE,
            None => {}
        }
        let score = eval(&ended, self.weights);
        let play = self.playout.map_or(0, |p| playout_value(ended, p));
        (score + play) as f64
    }

    /// The edge of node `id` that this world's action with `key` takes.
    fn own_action(&mut self, world: &Combat, key: u64) -> Option<Action> {
        if key == END_TURN_KEY {
            return Some(Action::EndTurn);
        }
        let legal = world.legal_actions();
        self.observer.observe(world, &mut self.obs);
        legal.into_iter().find(|a| self.edge_key(world, a) == key)
    }

    /// The PUCT edge of node `id`; `None` if it has none.
    fn select(&self, id: usize) -> Option<usize> {
        let node = &self.nodes[id];
        let edges = node.edges.as_ref()?;
        if edges.is_empty() {
            return None;
        }
        let scale = self.scale;
        let norm = |v: f64| v / scale;
        let parent = if node.visits > 0 {
            norm(node.total / f64::from(node.visits))
        } else {
            0.0
        };
        let sqrt_n = f64::from(node.visits.max(1)).sqrt();
        let mut best = 0;
        let mut best_score = f64::NEG_INFINITY;
        for (i, e) in edges.iter().enumerate() {
            let q = if e.visits == 0 {
                parent
            } else {
                norm(e.total / f64::from(e.visits))
            };
            let score = q + self.c * e.prior * sqrt_n / (1.0 + f64::from(e.visits));
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        Some(best)
    }

    /// Backs `value` (the leaf's) up `path`, each edge adding the bonuses below it.
    fn backup(&mut self, path: &[(usize, usize)], value: f64) {
        let mut v = value;
        for &(id, e) in path.iter().rev() {
            let node = &mut self.nodes[id];
            let edge = &mut node.edges.as_mut().expect("expanded")[e];
            v += edge.bonus;
            edge.visits += 1;
            edge.total += v;
            node.visits += 1;
            node.total += v;
        }
    }
}

/// How a turn's plays are grouped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// One tree node per play.
    Atomic,
    /// Whole-turn plans: RLB6's beam proposes plans in sampled worlds; each plan is played
    /// open-loop (by visible key) in the same `eval` fresh worlds and scored there.
    Plans,
}

/// The plan search's action among `legal`.
///
/// RLB6's beam proposes, in each of its sampled worlds, the
/// best plan of every first action; every distinct plan is then played by its visible keys in
/// `eval` other worlds (the same worlds for every plan), stopping where a key is not in hand,
/// and scored as the tree scores a leaf. A first action is worth its best plan's mean. No plan
/// is scored in the world that proposed it, so a draw it relied on is not taken for granted.
#[must_use]
pub fn choose_plans(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: TreeConfig,
    proposer: search::SearchConfig,
    playout_policy: Playout<'_>,
) -> Action {
    if combat.pending_decision().is_some()
        && let Some(answer) = answer_choice(combat)
    {
        return answer;
    }
    let first: Vec<Action> = legal
        .iter()
        .filter(|a| !matches!(a, Action::Resolve(_)))
        .cloned()
        .collect();
    if first.len() <= 1 {
        return first.into_iter().next().unwrap_or_else(|| legal[0].clone());
    }
    let before = search::player_hp(combat.state());
    let playouts = config.playouts && (context.big || config.playouts_all);
    let mut tree = Tree {
        nodes: Vec::new(),
        by_key: HashMap::new(),
        observer: Observer::new(true),
        obs: vec![0; OBS_BYTES],
        weights: Weights::of(context),
        potions_ok: context.big || before.0 * 100 < before.1 * 60,
        playout: playouts.then_some(playout_policy),
        c: config.c,
        config,
        prior: None,
        scratch: Scratch::default(),
        scale: 1.0,
    };
    let base = search::world_seed(combat);
    let gen_worlds = if context.boss {
        proposer.boss_worlds
    } else if context.big {
        proposer.big_worlds
    } else {
        proposer.worlds
    };
    // Each first action's distinct plans, as visible keys.
    let mut plans: Vec<Vec<Vec<u64>>> = vec![Vec::new(); first.len()];
    for k in 0..gen_worlds {
        let world = combat.blindfold_for_search(base ^ (k as u64).wrapping_mul(GOLDEN));
        let (scores, found) = search::world_plans(
            &world,
            context.big,
            &first,
            tree.weights,
            proposer.beam,
            None,
        );
        for (i, (score, plan)) in scores.into_iter().zip(found).enumerate() {
            if score <= search::LOST {
                continue;
            }
            let keys = tree.plan_keys(&world, &plan);
            if !plans[i].contains(&keys) {
                plans[i].push(keys);
            }
        }
    }
    let evals = config.sims_for(context).max(1);
    let worlds: Vec<Combat> = (0..evals)
        .map(|m| {
            combat.blindfold_for_search(
                base ^ 0xE7A1_0000_0000_0000 ^ u64::from(m + 1).wrapping_mul(GOLDEN),
            )
        })
        .collect();
    let mut best = (f64::NEG_INFINITY, 0);
    for (i, action_plans) in plans.iter().enumerate() {
        for keys in action_plans {
            let total: f64 = worlds
                .iter()
                .map(|w| tree.play_plan(w, &first[i], keys))
                .sum();
            let mean = total / f64::from(evals);
            if mean > best.0 {
                best = (mean, i);
            }
        }
    }
    first[best.1].clone()
}

impl Tree<'_> {
    /// The visible keys of `plan`'s actions, each in the position it is played in `world`.
    fn plan_keys(&mut self, world: &Combat, plan: &[Action]) -> Vec<u64> {
        let mut world = world.clone();
        let mut keys = Vec::with_capacity(plan.len());
        for action in plan {
            self.observer.observe(&world, &mut self.obs);
            keys.push(self.edge_key(&world, action));
            if world.apply(action.clone()).is_err() {
                break;
            }
            world = resolve_choices(world);
        }
        keys
    }

    /// The value of playing `first`, then the plan's other `keys` while this world has them,
    /// then ending the turn, in `world`.
    fn play_plan(&mut self, world: &Combat, first: &Action, keys: &[u64]) -> f64 {
        let mut world = world.clone();
        let mut bonus = 0.0;
        for (step, &key) in keys.iter().enumerate() {
            let action = if step == 0 {
                Some(first.clone())
            } else {
                self.own_action(&world, key)
            };
            let Some(action) = action else { break };
            if matches!(action, Action::EndTurn) {
                if world.apply(action).is_err() {
                    break;
                }
                return self.value(&resolve_choices(world), true);
            }
            let s = world.state();
            match &action {
                Action::UsePotion { .. } => bonus -= self.weights.potion as f64,
                Action::PlayCard { card, .. }
                    if s.card(*card).kind.card_type() == CardType::Power =>
                {
                    let left: i32 = s
                        .enemies
                        .iter()
                        .map(|&e| s.creature(e))
                        .filter(|c| c.is_alive())
                        .map(|c| c.hp + c.block)
                        .sum();
                    bonus += (POWER_BONUS + 2 * i64::from(left)) as f64;
                }
                _ => {}
            }
            if world.apply(action).is_err() {
                break;
            }
            world = resolve_choices(world);
            if world.state().result.is_some() {
                break;
            }
        }
        self.value(&world, false) + bonus
    }
}
