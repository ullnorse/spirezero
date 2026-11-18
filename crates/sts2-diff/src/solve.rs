//! `sts2-diff solve`: the exhaustive fight solver (`docs/specs/solver.md`).
//!
//! Expectimax over the engine's own steps: the player maximises the expected final HP (a death
//! ends at 0 HP); every chance event the step meets (a draw, a monster roll, a random pick) is
//! enumerated through the chance script ([`sts2_core::rng::chance`]) by re-running the step once
//! per outcome. Decision states are merged in a table keyed by
//! [`CombatState::canonical_hash`].
#![allow(
    clippy::float_arithmetic,
    clippy::suboptimal_flops,
    clippy::cast_precision_loss,
    reason = "probabilities and rates are f64 bookkeeping outside the game logic; plain a * b + c \
              keeps the merged and brute-force sums bit-comparable"
)]

use std::collections::HashMap;
use std::hash::Hash as _;

use sts2_core::Character;
use sts2_core::combat::{
    Action, CardId, Combat, CombatConfig, CombatState, CreatureId, Encounter, Hash128, Status,
};
use sts2_core::content::cards::CardKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::rng::chance::{self, Need};

use crate::Result;

/// Bounds on a node's expected final HP and death probability under the chosen policy. Exact
/// when no node below was cut off (`lo == hi`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Value {
    /// Lower bound on the expected final HP.
    pub hp_lo: f64,
    /// Upper bound on the expected final HP.
    pub hp_hi: f64,
    /// Lower bound on the death probability.
    pub death_lo: f64,
    /// Upper bound on the death probability.
    pub death_hi: f64,
}

impl Value {
    const fn exact(hp: f64, death: f64) -> Self {
        Self {
            hp_lo: hp,
            hp_hi: hp,
            death_lo: death,
            death_hi: death,
        }
    }

    const ZERO: Self = Self::exact(0.0, 0.0);

    /// The expected final HP's midpoint.
    #[must_use]
    pub const fn hp(&self) -> f64 {
        f64::midpoint(self.hp_lo, self.hp_hi)
    }

    /// The death probability's midpoint.
    #[must_use]
    pub const fn death(&self) -> f64 {
        f64::midpoint(self.death_lo, self.death_hi)
    }

    /// Whether the bounds meet.
    #[must_use]
    pub fn is_exact(&self) -> bool {
        self.hp_hi - self.hp_lo < 1e-9 && self.death_hi - self.death_lo < 1e-9
    }

    fn add_scaled(&mut self, p: f64, v: &Self) {
        self.hp_lo += p * v.hp_lo;
        self.hp_hi += p * v.hp_hi;
        self.death_lo += p * v.death_lo;
        self.death_hi += p * v.death_hi;
    }

    /// Whether `self` is a better choice than `other`: higher expected HP, then lower death.
    fn better_than(&self, other: &Self) -> bool {
        let (a, b) = (self.hp(), other.hp());
        if (a - b).abs() > 1e-9 {
            return a > b;
        }
        self.death() < other.death() - 1e-12
    }
}

/// Solver settings.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Merge identical states (the transposition table) and identical chance outcomes. Off is
    /// the brute-force check.
    pub merge: bool,
    /// Player turns past this are not expanded (bounded).
    pub max_turns: u32,
    /// The table's capacity: when full, the cheaper half of its entries is dropped.
    pub max_states: usize,
    /// The most HP the player can gain from any state to the fight's end. The default (a
    /// fight's max HP) always holds; a smaller one (0 with no healing content, 6 for Burning
    /// Blood alone) is checked on every explored transition and makes pruning far stronger.
    pub heal_bound: i32,
    /// The heal bound covers only the fight's end ([`heal_at_end`]): every transition but the
    /// winning one must not raise the player's HP, and the winning one by at most
    /// `heal_bound`; anything else is an error. This makes the bound hold along every explored
    /// line, not just per step.
    pub end_heal_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            merge: true,
            max_turns: 40,
            max_states: 20_000_000,
            heal_bound: i32::MAX / 2,
            end_heal_only: false,
        }
    }
}

/// Work counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Decision states expanded (distinct table entries when merging).
    pub states: u64,
    /// Decision-state visits, table hits included.
    pub visits: u64,
    /// Engine steps run (one per chance-script run).
    pub steps: u64,
    /// Nodes left unexpanded (turn cap): their bounds stay apart.
    pub cutoffs: u64,
    /// Nodes and chance nodes cut short by a better alternative (exactness unaffected).
    pub pruned: u64,
    /// Table entries dropped to make room (searched again when met).
    pub evicted: u64,
}

/// Tolerance on expected HP comparisons.
const EPS: f64 = 1e-9;

/// A table entry: the bounds found, and whether they were cut short by a bar (`alpha`) that a
/// later visit may not share.
#[derive(Clone, Copy, Debug)]
struct TableEntry {
    value: Value,
    pruned: bool,
    /// The turn cap it was searched under.
    cap: u32,
    /// Visits its search took: the cheaper entries go first when the table is full.
    work: u32,
    /// The best action's index in the node's action order: tried first when searched again.
    best: u8,
}

/// The solver and its table, shared across roots.
#[derive(Debug)]
pub struct Solver {
    config: Config,
    table: HashMap<u128, TableEntry, KeyHasher>,
    /// The current iteration's turn cap (iterative deepening up to `config.max_turns`).
    cap: u32,
    /// Work so far.
    pub stats: Stats,
    /// Spare combats whose buffers the next copies reuse.
    #[allow(
        clippy::vec_box,
        reason = "a combat is large inline: outcomes and the pool move boxes, not combats"
    )]
    pool: Vec<Box<Combat>>,
    /// [`draw_multisets`] by draw pile and count: a fight meets few distinct piles.
    multisets: HashMap<(Vec<u64>, u32), Multisets>,
}

/// A combat whose streams the chance script answers.
#[must_use]
pub fn scripted(combat: &Combat) -> Combat {
    let mut c = combat.clone();
    c.set_chance_scripted(true);
    c
}

/// The player creature's HP.
#[must_use]
pub fn player_hp(s: &CombatState) -> i32 {
    s.creature(s.player.creature).hp
}

impl Solver {
    /// A solver with an empty table.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            table: HashMap::default(),
            cap: config.max_turns,
            stats: Stats::default(),
            pool: Vec::new(),
            multisets: HashMap::new(),
        }
    }

    /// Runs `f` under turn caps from the current turn up to `config.max_turns` until `done`
    /// holds for its result (iterative deepening: a shallow search finds the strong lines that
    /// then prune the deep ones). Table entries stay valid across caps: they are bounds.
    fn deepen<T>(
        &mut self,
        turn: u32,
        mut f: impl FnMut(&mut Self) -> Result<T>,
        done: impl Fn(&T) -> bool,
    ) -> Result<T> {
        let mut cap = turn.min(self.config.max_turns);
        loop {
            self.cap = cap;
            let r = f(self)?;
            if done(&r) || cap >= self.config.max_turns {
                return Ok(r);
            }
            cap += 1;
        }
    }

    /// The value of every distinct legal action at `combat`'s decision.
    ///
    /// # Errors
    /// If the fight meets a chance event the script cannot enumerate, or unported content.
    pub fn action_values(&mut self, combat: &Combat) -> Result<Vec<(Action, Value)>> {
        let c = scripted(combat);
        let actions = distinct_actions(&c);
        self.deepen(
            c.state().player.turn_number,
            |me| {
                actions
                    .iter()
                    .map(|a| Ok((a.clone(), me.q(&c, a, f64::NEG_INFINITY)?)))
                    .collect()
            },
            |values: &Vec<(Action, Value)>| values.iter().all(|(_, v)| v.is_exact()),
        )
    }

    /// The value of `action` at `combat`'s decision.
    ///
    /// # Errors
    /// As [`Solver::action_values`].
    pub fn action_value(&mut self, combat: &Combat, action: &Action) -> Result<Value> {
        let c = scripted(combat);
        self.deepen(
            c.state().player.turn_number,
            |me| me.q(&c, action, f64::NEG_INFINITY),
            Value::is_exact,
        )
    }

    /// The value of `combat` under the optimal policy.
    ///
    /// # Errors
    /// As [`Solver::action_values`].
    pub fn value(&mut self, combat: &Combat) -> Result<Value> {
        let c = scripted(combat);
        self.deepen(
            c.state().player.turn_number,
            |me| me.node(&c, me.key(&c), f64::NEG_INFINITY),
            Value::is_exact,
        )
    }

    /// The optimal action at `combat`'s decision: the first of the best by expected HP, then
    /// death probability, once the best is exact and every other is exact or below it.
    ///
    /// # Errors
    /// As [`Solver::action_values`], or if there is no legal action.
    pub fn best_action(&mut self, combat: &Combat) -> Result<Action> {
        let c = scripted(combat);
        let actions = distinct_actions(&c);
        let pick = |values: &[(Action, Value)]| -> Option<usize> {
            let mut best: Option<usize> = None;
            for (i, (_, v)) in values.iter().enumerate() {
                if best.is_none_or(|b| v.better_than(&values[b].1)) {
                    best = Some(i);
                }
            }
            best
        };
        let values = self.deepen(
            c.state().player.turn_number,
            |me| {
                let mut values = Vec::new();
                let mut bar = f64::NEG_INFINITY;
                for a in &actions {
                    let v = me.q(&c, a, bar)?;
                    if v.is_exact() {
                        bar = bar.max(v.hp_lo);
                    }
                    values.push((a.clone(), v));
                }
                Ok(values)
            },
            |values: &Vec<(Action, Value)>| {
                pick(values).is_some_and(|b| {
                    let best = &values[b].1;
                    best.is_exact()
                        && values
                            .iter()
                            .all(|(_, v)| v.is_exact() || v.hp_hi < best.hp_lo - EPS)
                })
            },
        )?;
        pick(&values)
            .map(|b| values[b].0.clone())
            .ok_or_else(|| "no legal action".into())
    }

    /// The table key of `c`'s state: `None` when it is not merged.
    fn key(&self, c: &Combat) -> Option<u128> {
        (self.config.merge && c.status() == Status::PlayerTurn).then(|| c.state().canonical_hash())
    }

    /// An upper bound on the final HP reachable from `s`: its HP plus the configured heal
    /// allowance, at most its max HP.
    fn hp_bound(&self, s: &CombatState) -> f64 {
        let c = s.creature(s.player.creature);
        f64::from(
            c.max_hp
                .min(c.hp.max(0).saturating_add(self.config.heal_bound)),
        )
    }

    /// The bounds of a node that is not expanded: `[0, hp_bound]`.
    fn cut(&self, s: &CombatState) -> Value {
        Value {
            hp_lo: 0.0,
            hp_hi: self.hp_bound(s),
            death_lo: 0.0,
            death_hi: 1.0,
        }
    }

    /// A decision node (or a terminal one) of a scripted combat. The result bounds the node's
    /// value; it is tight (up to turn-cap and table-full cutoffs below) unless its upper bound is
    /// below `alpha`, where the caller has something better.
    fn node(&mut self, c: &Combat, key: Option<u128>, alpha: f64) -> Result<Value> {
        self.stats.visits += 1;
        let s = c.state();
        match c.status() {
            Status::Won => return Ok(Value::exact(f64::from(player_hp(s).max(0)), 0.0)),
            Status::Lost => return Ok(Value::exact(0.0, 1.0)),
            Status::Unsupported(u) => return Err(format!("unsupported content: {u:?}").into()),
            Status::PlayerTurn | Status::Decision => {}
        }
        if self.hp_bound(s) < alpha - EPS {
            self.stats.pruned += 1;
            return Ok(self.cut(s));
        }
        if s.player.turn_number > self.cap {
            self.stats.cutoffs += 1;
            return Ok(self.cut(s));
        }
        let entry = key.and_then(|k| self.table.get(&k));
        if let Some(e) = entry
            && (e.value.is_exact()
                || e.value.hp_hi < alpha - EPS
                || (!e.pruned && e.cap >= self.cap))
        {
            return Ok(e.value);
        }
        let first = entry.map(|e| e.best);
        let visits_before = self.stats.visits;
        let mut actions = distinct_actions(c);
        // Cards before ending the turn: a strong line first gives the others a bar to clear.
        // The best action of a shallower search goes first of all.
        actions.sort_by_key(|a| u8::from(matches!(a, Action::EndTurn)));
        if let Some(i) = first.map(usize::from).filter(|&i| i < actions.len()) {
            actions[..=i].rotate_right(1);
        }
        let mut best: Option<Value> = None;
        let mut best_at = 0;
        let mut pruned = false;
        for (i, action) in actions.iter().enumerate() {
            let bar = best.map_or(alpha, |b| alpha.max(b.hp_lo));
            let v = self.q(c, action, bar)?;
            if v.hp_hi < bar - EPS {
                pruned = true;
            }
            best = Some(best.map_or(v, |b| {
                let pick = if v.better_than(&b) {
                    best_at = i;
                    v
                } else {
                    b
                };
                Value {
                    hp_lo: b.hp_lo.max(v.hp_lo),
                    hp_hi: b.hp_hi.max(v.hp_hi),
                    ..pick
                }
            }));
        }
        // The index in the order before the rotation above.
        let best_index = match first.map(usize::from).filter(|&f| f < actions.len()) {
            Some(f) if best_at == 0 => f,
            Some(f) if best_at <= f => best_at - 1,
            _ => best_at,
        };
        let v = best.ok_or("a decision with no legal action")?;
        // Pruned below, but the node as a whole still clears `alpha`: its value is the best
        // action's, which was searched in full.
        let pruned = pruned && v.hp_hi < alpha - EPS;
        if let Some(k) = key {
            if self.table.len() >= self.config.max_states {
                self.evict();
            }
            let work = u32::try_from(self.stats.visits - visits_before).unwrap_or(u32::MAX);
            if self
                .table
                .insert(
                    k,
                    TableEntry {
                        value: v,
                        pruned,
                        cap: self.cap,
                        work,
                        best: u8::try_from(best_index).unwrap_or(0),
                    },
                )
                .is_none()
            {
                self.stats.states += 1;
            }
        } else {
            self.stats.states += 1;
        }
        Ok(v)
    }

    /// Makes room in a full table: drops the cheaper half of the entries (by the visits their
    /// search took), which are the cheapest to search again. Values stay exact: a dropped
    /// state is searched again when met.
    fn evict(&mut self) {
        let mut works: Vec<u32> = self.table.values().map(|e| e.work).collect();
        let mid = works.len() / 2;
        let (_, &mut bar, _) = works.select_nth_unstable(mid);
        let before = self.table.len();
        self.table.retain(|_, e| e.work > bar);
        if self.table.len() == before {
            self.table.clear();
        }
        self.stats.evicted += (before - self.table.len()) as u64;
    }

    /// The value of `action` at `c`: the chance-weighted value of its outcomes, cut short once
    /// it cannot reach `alpha`.
    fn q(&mut self, c: &Combat, action: &Action, alpha: f64) -> Result<Value> {
        let outcomes = self.expand(c, action)?;
        let v = self.q_outcomes(c, &outcomes, alpha);
        self.pool.extend(outcomes.into_iter().map(|o| o.combat));
        v
    }

    fn q_outcomes(&mut self, c: &Combat, outcomes: &[Outcome], alpha: f64) -> Result<Value> {
        let parent_hp = player_hp(c.state());
        let mut rest: f64 = 0.0;
        let mut rest_p: f64 = 0.0;
        for o in outcomes {
            let s = o.combat.state();
            let hp = player_hp(s);
            let limit = if self.config.end_heal_only && o.combat.status() != Status::Won {
                parent_hp
            } else {
                parent_hp.saturating_add(self.config.heal_bound)
            };
            if hp > limit {
                return Err(format!(
                    "HP rose from {parent_hp} to {hp}, past the heal bound {}{}; pass \
                     --heal-bound N",
                    self.config.heal_bound,
                    if self.config.end_heal_only {
                        " (end of the fight only)"
                    } else {
                        ""
                    }
                )
                .into());
            }
        }
        // Each outcome's upper bound: its HP bound, or a table entry's if tighter. Likely
        // outcomes first: they move the sum the most.
        let mut order: Vec<(usize, f64)> = outcomes
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let mut ub = self.hp_bound(o.combat.state());
                if let Some(e) = o.key.and_then(|k| self.table.get(&k)) {
                    ub = ub.min(e.value.hp_hi);
                }
                (i, ub)
            })
            .collect();
        order.sort_by(|x, y| outcomes[y.0].p.total_cmp(&outcomes[x.0].p));
        for &(i, ub) in &order {
            rest += outcomes[i].p * ub;
            rest_p += outcomes[i].p;
        }
        let mut v = Value::ZERO;
        for (i, ub) in order {
            let o = &outcomes[i];
            let p = o.p;
            rest -= p * ub;
            rest_p -= p;
            // The child must reach this for the action to reach `alpha`.
            let need = (alpha - v.hp_hi - rest) / p;
            let cv = self.node(&o.combat, o.key, need)?;
            v.add_scaled(p, &cv);
            if v.hp_hi + rest < alpha - EPS {
                self.stats.pruned += 1;
                v.hp_hi += rest.max(0.0);
                v.death_hi += rest_p.max(0.0);
                return Ok(v);
            }
        }
        Ok(v)
    }

    /// A copy of `c` from the pool of spare combats.
    fn copy(&mut self, c: &Combat) -> Box<Combat> {
        self.pool.pop().map_or_else(
            || Box::new(c.clone()),
            |mut x| {
                (*x).clone_from(c);
                x
            },
        )
    }

    /// Every outcome of `action` at `c` with its probability: the step re-run once per chance
    /// outcome, identical outcomes merged when merging is on.
    fn expand(&mut self, c: &Combat, action: &Action) -> Result<Vec<Outcome>> {
        let mut out: Vec<Outcome> = Vec::with_capacity(8);
        let mut stack: Vec<(Vec<u32>, f64)> = vec![(Vec::new(), 1.0)];
        chance::set_batch_draws(self.config.merge);
        while let Some((script, p)) = stack.pop() {
            chance::install(&script);
            let mut child = self.copy(c);
            child
                .apply(action.clone())
                .map_err(|e| format!("{action:?} rejected: {e:?}"))?;
            self.stats.steps += 1;
            let Some(need) = chance::take_need() else {
                let key = self.key(&child);
                match key.and_then(|k| out.iter_mut().find(|o| o.key == Some(k))) {
                    Some(o) => {
                        o.p += p;
                        self.pool.push(child);
                    }
                    None => out.push(Outcome {
                        combat: child,
                        p,
                        key,
                    }),
                }
                continue;
            };
            self.pool.push(child);
            let mut branch = |ks: &[u32], q: f64| {
                let mut s = Vec::with_capacity(script.len() + ks.len());
                s.extend_from_slice(&script);
                s.extend_from_slice(ks);
                stack.push((s, p * q));
            };
            match need {
                Need::Uniform(n) => {
                    let q = 1.0 / f64::from(n);
                    for k in 0..n {
                        branch(&[k], q);
                    }
                }
                Need::Weights(w) => {
                    let total: f64 = w.iter().map(|&x| f64::from(x.max(0.0))).sum();
                    for (k, &x) in (0u32..).zip(&w) {
                        if x > 0.0 {
                            branch(&[k], f64::from(x) / total);
                        }
                    }
                }
                Need::Draw { keys, count, .. } => {
                    if count <= 1 {
                        // One card: once per distinct card (every card without merging).
                        let len = keys.len() as f64;
                        let mut classes: Vec<(u64, u32, u32)> = Vec::new();
                        for (i, &key) in (0u32..).zip(&keys) {
                            match classes.iter_mut().find(|(k, _, _)| *k == key) {
                                Some((_, _, n)) => *n += 1,
                                None => classes.push((key, i, 1)),
                            }
                        }
                        for (_, first, n) in classes {
                            branch(&[first], f64::from(n) / len);
                        }
                    } else {
                        let multisets = self
                            .multisets
                            .entry((keys, count))
                            .or_insert_with_key(|(keys, count)| draw_multisets(keys, *count));
                        for (ks, q) in multisets.iter() {
                            branch(ks, *q);
                        }
                    }
                }
                Need::Unsupported(what) => {
                    return Err(format!("chance event the solver cannot enumerate: {what}").into());
                }
            }
        }
        Ok(out)
    }

    /// The number of distinct states in the table.
    #[must_use]
    pub fn table_len(&self) -> usize {
        self.table.len()
    }
}

/// The table's hasher: its keys are already well-mixed 128-bit hashes, so their low word
/// is the bucket hash.
#[derive(Clone, Copy, Default)]
struct KeyHasher;

impl std::hash::BuildHasher for KeyHasher {
    type Hasher = KeyHash;
    fn build_hasher(&self) -> KeyHash {
        KeyHash(0)
    }
}

/// [`KeyHasher`]'s hasher: the low word of the `u128` key.
#[derive(Default)]
struct KeyHash(u64);

impl std::hash::Hasher for KeyHash {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.rotate_left(8) ^ u64::from(b);
        }
    }

    fn write_u128(&mut self, i: u128) {
        self.0 = u64::try_from(i & u128::from(u64::MAX)).unwrap_or_default();
    }
}

/// An outcome of an action: the combat, its probability and its table key (merged states).
struct Outcome {
    combat: Box<Combat>,
    p: f64,
    key: Option<u128>,
}

/// The multisets [`draw_multisets`] lists: each one's script and probability.
type Multisets = Vec<(Vec<u32>, f64)>;

/// `n` choose `k`, exact for the pile sizes of a fight.
fn binomial(n: u32, k: u32) -> u128 {
    let k = k.min(n - k);
    (0..u128::from(k)).fold(1, |acc, i| acc * (u128::from(n) - i) / (i + 1))
}

/// Calls `f` with every split `h` of `left` cards over `classes[i..]` (each at most its count;
/// `room[i]` is the count of `classes[i..]`).
fn splits(
    i: usize,
    left: u32,
    classes: &[(u64, u32)],
    room: &[u32],
    h: &mut [u32],
    f: &mut dyn FnMut(&[u32]),
) {
    if i == classes.len() {
        if left == 0 {
            f(h);
        }
        return;
    }
    let lo = left.saturating_sub(room[i + 1]);
    for k in lo..=left.min(classes[i].1) {
        h[i] = k;
        splits(i + 1, left - k, classes, room, h, f);
    }
    h[i] = 0;
}

/// Every multiset of `count` cards drawn from a pile with content keys `keys`.
///
/// Each comes with its hypergeometric probability `prod C(c_i, h_i) / C(N, count)` and the
/// script that draws it: one draw-pile index per draw, taking each class's cards in the
/// pile's order.
fn draw_multisets(keys: &[u64], count: u32) -> Multisets {
    let mut classes: Vec<(u64, u32)> = Vec::new();
    for &key in keys {
        match classes.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => classes.push((key, 1)),
        }
    }
    let total = binomial(u32::try_from(keys.len()).unwrap_or(u32::MAX), count) as f64;
    let mut out = Vec::new();
    let mut h = vec![0u32; classes.len()];
    // Classes after `i` can hold at most `room[i]` cards.
    let mut room = vec![0u32; classes.len() + 1];
    for i in (0..classes.len()).rev() {
        room[i] = room[i + 1] + classes[i].1;
    }
    splits(0, count, &classes, &room, &mut h, &mut |h| {
        let ways: f64 = classes
            .iter()
            .zip(h)
            .map(|(&(_, c), &k)| binomial(c, k) as f64)
            .product();
        let mut pile = keys.to_vec();
        let mut script = Vec::with_capacity(count as usize);
        for (&(key, _), &k) in classes.iter().zip(h) {
            for _ in 0..k {
                let at = pile.iter().position(|&x| x == key).unwrap_or(0);
                pile.remove(at);
                script.push(u32::try_from(at).unwrap_or(u32::MAX));
            }
        }
        out.push((script, ways / total));
    });
    out
}

/// The heal the player's relics name: the sum of their `Heal` vars (Burning Blood's 6).
///
/// It is the automatic heal bound, held to the fight's end only ([`Config::end_heal_only`]): a
/// relic that heals mid-fight, or by more, fails the solve instead of bounding it wrongly.
#[must_use]
pub fn heal_at_end(s: &CombatState) -> i32 {
    s.player
        .relics
        .iter()
        .flat_map(|r| sts2_core::content::tables::relic_vars(r.kind.id()))
        .filter(|v| v.name == "Heal")
        .filter_map(|v| v.value)
        .map(sts2_core::decimal::Decimal::to_int_truncating)
        .sum()
}

/// The legal actions with identical cards in hand (same content, same target) and identical
/// potions tried once.
#[must_use]
pub fn distinct_actions(c: &Combat) -> Vec<Action> {
    let s = c.state();
    let mut seen: Vec<(u8, u128, Option<CreatureId>)> = Vec::new();
    let mut out = Vec::new();
    let mut cards = Vec::new();
    s.card_hashes(&mut cards);
    c.for_each_legal_action(|a| {
        let key = match &a {
            Action::PlayCard { card, target } => Some((0, cards[card.index()], *target)),
            Action::UsePotion { slot, target } => s.player.potions[*slot].map(|p| {
                let mut h = Hash128::new(0x907);
                p.kind.hash(&mut h);
                (1, h.finish128(), *target)
            }),
            Action::EndTurn | Action::Resolve(_) => None,
        };
        if let Some(key) = key {
            if seen.contains(&key) {
                return;
            }
            seen.push(key);
        }
        out.push(a);
    });
    out
}

/// Swaps two cards' contents (everything but their labels and places): a set-up helper for an
/// opening hand.
pub fn swap_card_contents(s: &mut CombatState, first: CardId, second: CardId) {
    let (one, two) = (*s.card(first), *s.card(second));
    for (id, from) in [(first, two), (second, one)] {
        let c = &mut s.cards[id.0 as usize];
        c.kind = from.kind;
        c.upgrade_level = from.upgrade_level;
        c.keywords = from.keywords;
        c.energy_cost = from.energy_cost;
        c.star_cost = from.star_cost;
        c.exhaust_on_next_play = from.exhaust_on_next_play;
        c.single_turn_retain = from.single_turn_retain;
        c.single_turn_sly = from.single_turn_sly;
        c.affliction = from.affliction;
        c.enchantment = from.enchantment;
        c.base_replay_count = from.base_replay_count;
        c.data = from.data;
        c.is_dupe = from.is_dupe;
        c.stale_calculated = from.stale_calculated;
    }
}

/// A fight's start for the solver.
#[derive(Clone, Debug)]
pub struct Setup {
    /// The character, with its starter deck and relic.
    pub character: Character,
    /// The encounter.
    pub encounter: Encounter,
    /// The ascension.
    pub ascension: u8,
    /// The seed (decides the start: enemy HP, the opening hand unless `hand` is given).
    pub seed: String,
    /// The opening hand by card kind, if forced.
    pub hand: Vec<CardKind>,
    /// Keep the character's starter relic.
    pub starter_relic: bool,
    /// Relics added to the starter relic.
    pub relics: Vec<RelicKind>,
    /// Every enemy's HP, if forced.
    pub enemy_hp: Option<i32>,
    /// The player's HP, if forced.
    pub player_hp: Option<i32>,
}

impl Setup {
    /// `character` at A0 against `encounter`, seed `SOLVE`.
    #[must_use]
    pub fn new(character: Character, encounter: Encounter) -> Self {
        Self {
            character,
            encounter,
            ascension: 0,
            seed: "SOLVE".to_owned(),
            hand: Vec::new(),
            starter_relic: true,
            relics: Vec::new(),
            enemy_hp: None,
            player_hp: None,
        }
    }

    /// The combat at its first decision.
    ///
    /// # Errors
    /// Unported starter content, or a hand the deck cannot give.
    pub fn start(&self) -> Result<Combat> {
        let mut config = CombatConfig::new(self.character, &self.seed, self.ascension)
            .map_err(|u| format!("unsupported starter content: {u:?}"))?
            .with_encounter(self.encounter);
        if !self.starter_relic {
            config.relics.clear();
        }
        config.relics.extend(
            self.relics
                .iter()
                .map(|&r| sts2_core::content::relics::OwnedRelic::from(r)),
        );
        if let Some(hp) = self.player_hp {
            config.player_hp = hp;
        }
        let mut combat = Combat::new(config);
        let hand = self.hand.clone();
        let enemy_hp = self.enemy_hp;
        combat.edit(|s| -> Result<()> {
            if !hand.is_empty() {
                set_hand(s, &hand)?;
            }
            if let Some(hp) = enemy_hp {
                for id in s.enemies.clone() {
                    let c = &mut s.creatures[id.0 as usize];
                    c.hp = hp;
                    c.max_hp = c.max_hp.max(hp);
                }
            }
            Ok(())
        })?;
        Ok(combat)
    }
}

/// Gives the hand the kinds `hand` (as many cards as it holds now) by swapping card contents
/// with the draw pile.
fn set_hand(s: &mut CombatState, hand: &[CardKind]) -> Result<()> {
    if hand.len() != s.player.hand.len() {
        return Err(format!(
            "--hand names {} cards, the opening hand holds {}",
            hand.len(),
            s.player.hand.len()
        )
        .into());
    }
    for (i, &kind) in hand.iter().enumerate() {
        let here = s.player.hand[i];
        if s.card(here).kind == kind {
            continue;
        }
        let donor = s.player.hand[i + 1..]
            .iter()
            .chain(s.player.draw.iter())
            .copied()
            .find(|&c| s.card(c).kind == kind)
            .ok_or_else(|| format!("the deck has no spare {kind:?} for the hand"))?;
        swap_card_contents(s, here, donor);
    }
    Ok(())
}

/// Plays the first hand card of `kind` (targeting the first enemy if it needs one).
///
/// # Errors
/// If no such card is playable.
pub fn play_kind(combat: &mut Combat, kind: CardKind) -> Result<Status> {
    let action = combat
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::PlayCard { card, .. } if combat.state().card(*card).kind == kind))
        .ok_or_else(|| format!("no playable {kind:?} in hand"))?;
    combat
        .apply(action)
        .map_err(|e| format!("{kind:?} rejected: {e:?}").into())
}

/// Plays the solver's policy in the unscripted engine from `start` with its hidden state
/// redrawn from `seed` ([`Combat::determinize`]); returns the final HP (0 on a death).
///
/// # Errors
/// As [`Solver::action_values`].
pub fn play_policy(solver: &mut Solver, start: &Combat, seed: u64) -> Result<i32> {
    let mut c = start.clone();
    c.determinize(seed);
    loop {
        match c.status() {
            Status::Won => return Ok(player_hp(c.state()).max(0)),
            Status::Lost => return Ok(0),
            Status::Unsupported(u) => return Err(format!("unsupported content: {u:?}").into()),
            Status::PlayerTurn | Status::Decision => {
                let a = solver.best_action(&c)?;
                c.apply(a)
                    .map_err(|e| format!("policy action rejected: {e:?}"))?;
            }
        }
    }
}

/// A short label for an action.
#[must_use]
pub fn label(c: &Combat, action: &Action) -> String {
    match action {
        Action::PlayCard { card, target } => {
            let card = c.state().card(*card);
            let plus = if card.upgrade_level > 0 { "+" } else { "" };
            target.map_or_else(
                || format!("{}{plus}", card.kind.name()),
                |t| format!("{}{plus} -> #{}", card.kind.name(), t.0),
            )
        }
        Action::UsePotion { slot, .. } => format!("potion {slot}"),
        Action::EndTurn => "End turn".to_owned(),
        Action::Resolve(d) => format!("{d:?}"),
    }
}

/// The per-action table `sts2-diff solve` prints, one line at a time to `out`: expected HP loss
/// from the root's HP, death probability, and the work each action's solve took.
///
/// # Errors
/// As [`Solver::action_values`].
pub fn report(solver: &mut Solver, combat: &Combat, mut out: impl FnMut(&str)) -> Result<()> {
    let root_hp = f64::from(player_hp(combat.state()));
    let c = scripted(combat);
    out(&format!(
        "{:<28} {:>12} {:>8} {:>10} {:>11} {:>11} {:>11}",
        "action", "E[HP loss]", "death%", "states", "visits", "states/s", "visits/s"
    ));
    for action in distinct_actions(&c) {
        let before = solver.stats;
        let t = std::time::Instant::now();
        let v = solver.action_value(&c, &action)?;
        let secs = t.elapsed().as_secs_f64().max(1e-9);
        let states = solver.stats.states - before.states;
        let visits = solver.stats.visits - before.visits;
        let loss = if v.is_exact() {
            format!("{:.6}", root_hp - v.hp())
        } else {
            format!("[{:.2},{:.2}]", root_hp - v.hp_hi, root_hp - v.hp_lo)
        };
        out(&format!(
            "{:<28} {:>12} {:>8.3} {:>10} {:>11} {:>11.0} {:>11.0}",
            label(&c, &action),
            loss,
            100.0 * v.death(),
            states,
            visits,
            states as f64 / secs,
            visits as f64 / secs,
        ));
    }
    Ok(())
}

/// The totals line: counts, time and single-thread rates.
#[must_use]
pub fn totals(st: &Stats, elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs_f64();
    format!(
        "total: {} states, {} visits, {} steps, {} cutoffs, {} evicted in {secs:.2} s ({:.0} states/s, {:.0} visits/s)",
        st.states,
        st.visits,
        st.steps,
        st.cutoffs,
        st.evicted,
        st.states as f64 / secs,
        st.visits as f64 / secs
    )
}

/// Plays the solver's policy `games` times in the unscripted engine (seeds `0..games`) and
/// compares the mean HP loss, with its 99% confidence interval, to the solver's expectation.
///
/// # Errors
/// When the solve or a game fails.
pub fn monte_carlo(solver: &mut Solver, combat: &Combat, games: u64) -> Result<String> {
    let root = solver.value(combat)?;
    let hp0 = f64::from(player_hp(combat.state()));
    let losses = (0..games)
        .map(|seed| Ok(hp0 - f64::from(play_policy(solver, combat, seed)?)))
        .collect::<Result<Vec<f64>>>()?;
    let n = games as f64;
    let mean = losses.iter().sum::<f64>() / n;
    let var = losses.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    Ok(format!(
        "monte carlo: {games} games, mean loss {mean:.4} +- {:.4} (99% CI), solver {:.4}",
        2.576 * (var / n).sqrt(),
        hp0 - root.hp()
    ))
}
