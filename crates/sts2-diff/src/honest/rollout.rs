//! A learned playout policy (RLB7): a per-card softmax over the hand and the end of the turn.
//!
//! A candidate is a distinct card in hand (its kind and whether it is upgraded; an attack aims at
//! the weakest enemy, as the priority list does) or ending the turn. Its logit is
//! `w[key] · f(s) + v · g(a)`: `f` is a few visible features of the position (HP, block, energy,
//! the intents' damage, powers, pile sizes) and `g` a few of the candidate's (cost, type, its
//! target's share of the enemy HP). Everything it reads the player sees.
//!
//! It is fitted first to the search's own moves ([`fit`]), then improved by REINFORCE on playouts
//! from fight starts ([`reinforce`]). The priority list stays as the fallback when no weights
//! are loaded.
//!
//! The v2 net (RLB14) reads [`F2`] position features (v1's and the powers, piles and attackers
//! that decide boss fights) and adds a hidden layer: `w[key] · [f, relu(A f + b)] + v · g`.
//! The v3 net (RLB15) also reads [`F3`]: which elite or boss fight it is ([`ENCOUNTERS`]).
//! The v4 net (RLB17) adds the hand to the hidden layer: `relu(A f + b + Σ e[key in hand])`.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    reason = "the policy's forward pass and training; it decides no game rule: small counts and HP become floats, and exactness does not matter"
)]

use sts2_core::Combat;
use sts2_core::combat::{Action, CombatResult, CreatureId};
use sts2_core::content::cards::{CardKind, CardType};
use sts2_core::content::powers::PowerKind;
use sts2_rl::intents::IntentCache;
use sts2_rl::vocab;

use super::search::answer_choice;

/// Position features of the v1 net.
pub const F: usize = 24;
/// Position features of the v2 net: v1's, then [`ENEMY_POWERS`], [`PLAYER_POWERS`] and 8 more.
pub const F2: usize = F + ENEMY_POWERS.len() + PLAYER_POWERS.len() + 8;
/// Position features of the v3 net: v2's, then one per [`ENCOUNTERS`] (the fight it is).
pub const F3: usize = F2 + ENCOUNTERS.len();
/// The elite and boss fights the v3 net tells apart (the player sees which fight it is).
const ENCOUNTERS: [&str; 24] = [
    "QUEEN_BOSS",
    "TEST_SUBJECT_BOSS",
    "AEONGLASS_BOSS",
    "KNIGHTS_ELITE",
    "MECHA_KNIGHT_ELITE",
    "SOUL_NEXUS_ELITE",
    "THE_INSATIABLE_BOSS",
    "KAISER_CRAB_BOSS",
    "KNOWLEDGE_DEMON_BOSS",
    "INFESTED_PRISMS_ELITE",
    "DECIMILLIPEDE_ELITE",
    "ENTOMANCER_ELITE",
    "CEREMONIAL_BEAST_BOSS",
    "LAGAVULIN_MATRIARCH_BOSS",
    "SOUL_FYSH_BOSS",
    "THE_KIN_BOSS",
    "VANTOM_BOSS",
    "WATERFALL_GIANT_BOSS",
    "BYGONE_EFFIGY_ELITE",
    "BYRDONIS_ELITE",
    "PHANTASMAL_GARDENERS_ELITE",
    "PHROG_PARASITE_ELITE",
    "SKULKING_COLONY_ELITE",
    "TERROR_EEL_ELITE",
];
/// Enemy powers the v2 net reads (summed over the living enemies, at most 5 each).
const ENEMY_POWERS: [PowerKind; 12] = [
    PowerKind::Intangible,
    PowerKind::Artifact,
    PowerKind::Thorns,
    PowerKind::Plating,
    PowerKind::Ritual,
    PowerKind::Slippery,
    PowerKind::Reflect,
    PowerKind::CurlUp,
    PowerKind::Enrage,
    PowerKind::Minion,
    PowerKind::HardenedShell,
    PowerKind::Poison,
];
/// Player powers the v2 net reads (at most 5 each).
const PLAYER_POWERS: [PowerKind; 16] = [
    PowerKind::DemonForm,
    PowerKind::Barricade,
    PowerKind::Corruption,
    PowerKind::FeelNoPain,
    PowerKind::DarkEmbrace,
    PowerKind::Juggernaut,
    PowerKind::Rage,
    PowerKind::Vigor,
    PowerKind::Inferno,
    PowerKind::Rupture,
    PowerKind::FlameBarrier,
    PowerKind::Buffer,
    PowerKind::Regen,
    PowerKind::NoDraw,
    PowerKind::Intangible,
    PowerKind::Plating,
];
/// Candidate features.
pub const G: usize = 6;
/// The key of ending the turn (vocabulary id 0 is padding, so no card has it).
const END: usize = 0;
/// The export's magic bytes.
const MAGIC: &[u8; 8] = b"STS2RLP1";
/// The v2 export's magic bytes.
const MAGIC2: &[u8; 8] = b"STS2RLP2";
/// The v4 export's magic bytes (v2's, then `e`).
const MAGIC3: &[u8; 8] = b"STS2RLP3";

/// The number of candidate keys: two per card vocabulary id.
fn key_count() -> usize {
    CardKind::ALL
        .into_iter()
        .map(|k| usize::from(vocab::card(k)))
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        * 2
}

/// The policy's weights.
#[derive(Clone, Debug)]
pub struct RolloutNet {
    keys: usize,
    /// The position features read ([`F`], [`F2`] or [`F3`]).
    nf: usize,
    /// The hidden units (0 for v1).
    nh: usize,
    /// `keys × (nf + nh)`, row-major.
    w: Vec<f32>,
    v: [f32; G],
    /// The hidden layer: `nh × nf`, row-major, and its bias.
    a: Vec<f32>,
    b: Vec<f32>,
    /// The v4 hand input: `keys × nh`, row-major (empty before v4).
    e: Vec<f32>,
}

/// A candidate move.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub key: usize,
    pub g: [f32; G],
    pub action: Action,
}

/// One position for training: its features, candidates and the move taken.
#[derive(Clone, Debug)]
pub struct Sample {
    pub f: [f32; F3],
    /// The keys of the cards in hand (read by a v4 net).
    pub hand: Vec<usize>,
    pub candidates: Vec<(usize, [f32; G])>,
    pub chosen: usize,
}

/// Scratch for the policy's calls.
#[derive(Debug, Default)]
pub struct Scratch {
    intents: IntentCache,
    candidates: Vec<Candidate>,
    logits: Vec<f32>,
    z: Vec<f32>,
}

/// The visible features of `combat`'s position.
#[must_use]
pub fn features(combat: &Combat, cache: &mut IntentCache) -> [f32; F3] {
    let h = combat.hooked();
    let s = h.s;
    let me = s.creature(s.player.creature);
    let mut incoming = 0_i32;
    cache.begin();
    let mut alive = 0;
    let mut enemy_hp = 0;
    let mut min_enemy = i32::MAX;
    let (mut e_vuln, mut e_weak, mut e_str) = (0, 0, 0);
    let mut e_powers = [0_i32; ENEMY_POWERS.len()];
    let (mut attackers, mut top_attack, mut e_block) = (0, 0, 0);
    for &e in &s.enemies {
        let c = s.creature(e);
        if !c.is_alive() {
            continue;
        }
        alive += 1;
        enemy_hp += c.hp + c.block;
        e_block += c.block;
        min_enemy = min_enemy.min(c.hp + c.block);
        e_vuln += c.power_amount(PowerKind::Vulnerable);
        e_weak += c.power_amount(PowerKind::Weak);
        e_str += c.power_amount(PowerKind::Strength);
        for (n, &kind) in e_powers.iter_mut().zip(&ENEMY_POWERS) {
            *n += c.power_amount(kind);
        }
        let mut damage = 0;
        for intent in cache.intents(h, e) {
            if let Some(a) = intent.attack {
                damage += a.damage * i32::try_from(a.hits).unwrap_or(1);
            }
        }
        incoming += damage;
        attackers += i32::from(damage > 0);
        top_attack = top_attack.max(damage);
    }
    if min_enemy == i32::MAX {
        min_enemy = 0;
    }
    let hp = me.hp as f32;
    let max_hp = me.max_hp.max(1) as f32;
    let unblocked = (incoming - me.block).max(0) as f32;
    let p = &s.player;
    let v1 = [
        1.0,
        hp / max_hp,
        me.block as f32 / 20.0,
        p.energy as f32 / 3.0,
        incoming as f32 / 20.0,
        unblocked / 20.0,
        alive as f32 / 3.0,
        enemy_hp as f32 / 100.0,
        min_enemy as f32 / 50.0,
        p.hand.len() as f32 / 10.0,
        p.draw.len() as f32 / 20.0,
        p.turn_number.min(30) as f32 / 10.0,
        me.power_amount(PowerKind::Strength) as f32 / 5.0,
        me.power_amount(PowerKind::Dexterity) as f32 / 5.0,
        me.power_amount(PowerKind::Vulnerable).min(3) as f32 / 3.0,
        me.power_amount(PowerKind::Weak).min(3) as f32 / 3.0,
        me.power_amount(PowerKind::Frail).min(3) as f32 / 3.0,
        e_vuln.min(6) as f32 / 3.0,
        e_weak.min(6) as f32 / 3.0,
        e_str as f32 / 5.0,
        f32::from(incoming > 0),
        f32::from(p.energy == 0),
        f32::from(hp < 0.3 * max_hp),
        f32::from(unblocked >= hp),
    ];
    let mut out = [0.0; F3];
    out[..F].copy_from_slice(&v1);
    let powers = e_powers
        .iter()
        .copied()
        .chain(PLAYER_POWERS.iter().map(|&k| me.power_amount(k)));
    for (x, n) in out[F..].iter_mut().zip(powers) {
        *x = n.clamp(0, 5) as f32 / 5.0;
    }
    let rest = F + ENEMY_POWERS.len() + PLAYER_POWERS.len();
    out[rest..F2].copy_from_slice(&[
        hp / 100.0,
        enemy_hp as f32 / 300.0,
        e_block as f32 / 20.0,
        attackers as f32 / 2.0,
        top_attack as f32 / 20.0,
        p.max_energy as f32 / 3.0,
        p.discard.len() as f32 / 20.0,
        p.exhaust.len() as f32 / 10.0,
    ]);
    if let Some(i) = encounter_index(s.encounter) {
        out[F2 + i] = 1.0;
    }
    out
}

/// `encounter`'s place in [`ENCOUNTERS`], if it is one of them.
fn encounter_index(encounter: sts2_core::combat::Encounter) -> Option<usize> {
    static IDS: std::sync::OnceLock<[u16; ENCOUNTERS.len()]> = std::sync::OnceLock::new();
    let ids = IDS.get_or_init(|| ENCOUNTERS.map(vocab::encounter_id));
    let id = vocab::encounter(encounter);
    ids.iter().position(|&x| x == id)
}

/// The candidates among `legal` into `out`: one per card key in hand (the first such play,
/// aimed at the weakest target), and ending the turn when it is legal. Potions and choices are
/// not the playout's.
pub fn candidates(combat: &Combat, legal: &[Action], out: &mut Vec<Candidate>) {
    out.clear();
    let s = combat.state();
    let total: i32 = s
        .enemies
        .iter()
        .map(|&e| s.creature(e))
        .filter(|c| c.is_alive())
        .map(|c| c.hp + c.block)
        .sum::<i32>()
        .max(1);
    let target_hp = |t: Option<CreatureId>| t.map_or(0, |t| s.creature(t).hp + s.creature(t).block);
    for action in legal {
        let Action::PlayCard { card, target } = action else {
            continue;
        };
        let c = s.card(*card);
        let key = usize::from(vocab::card(c.kind)) * 2 + usize::from(c.upgrade_level > 0);
        if let Some(i) = out.iter().position(|x| x.key == key) {
            // The same card at a weaker target replaces it.
            let Action::PlayCard { target: old, .. } = out[i].action else {
                continue;
            };
            if target.is_some() && target_hp(*target) < target_hp(old) {
                out[i].action = action.clone();
                out[i].g[5] = target_hp(*target) as f32 / total as f32;
            }
            continue;
        }
        let kind = c.kind.card_type();
        out.push(Candidate {
            key,
            g: [
                combat.hooked().energy_cost(*card) as f32 / 3.0,
                f32::from(kind == CardType::Attack),
                f32::from(kind == CardType::Skill),
                f32::from(kind == CardType::Power),
                f32::from(matches!(kind, CardType::Status | CardType::Curse)),
                if target.is_some() {
                    target_hp(*target) as f32 / total as f32
                } else {
                    0.0
                },
            ],
            action: action.clone(),
        });
    }
    if legal.contains(&Action::EndTurn) {
        out.push(Candidate {
            key: END,
            g: [0.0; G],
            action: Action::EndTurn,
        });
    }
}

/// The committed weights (`data/rollout.bin`; see `data/rollout.md` for their provenance).
const BUILTIN: &[u8] = include_bytes!("../../data/rollout.bin");

/// RLB14's v2 weights (`data/rollout-v2.bin`; see `data/rollout.md`).
const V2: &[u8] = include_bytes!("../../data/rollout-v2.bin");

impl RolloutNet {
    /// The committed weights, or zero weights if they are for another vocabulary.
    #[must_use]
    pub fn builtin() -> Self {
        Self::from_bytes(BUILTIN).unwrap_or_default()
    }

    /// RLB14's committed v2 weights (the boss-leaf playouts), or zero weights if they are for
    /// another vocabulary.
    #[must_use]
    pub fn v2() -> Self {
        Self::from_bytes(V2).unwrap_or_default()
    }

    /// Zero weights (uniform play).
    #[must_use]
    pub fn new() -> Self {
        let keys = key_count();
        Self {
            keys,
            nf: F,
            nh: 0,
            w: vec![0.0; keys * F],
            v: [0.0; G],
            a: Vec::new(),
            b: Vec::new(),
            e: Vec::new(),
        }
    }

    /// A v2 net of `hidden` units that plays as `self` does: `self`'s weights on the v1
    /// features, the rest zero, and the hidden layer small and random (from `seed`).
    #[must_use]
    pub fn widened(&self, hidden: usize, seed: u64) -> Self {
        let width = F2 + hidden;
        let mut w = vec![0.0; self.keys * width];
        for key in 0..self.keys {
            let old = self.nf + self.nh;
            w[key * width..key * width + self.nf]
                .copy_from_slice(&self.w[key * old..key * old + self.nf]);
        }
        let mut rng = seed;
        let a = (0..hidden * F2)
            .map(|_| (unit(&mut rng) - 0.5) * 0.6)
            .collect();
        Self {
            keys: self.keys,
            nf: F2,
            nh: hidden,
            w,
            v: self.v,
            a,
            b: vec![0.0; hidden],
            e: Vec::new(),
        }
    }

    /// A v3 net that plays as `self` (a v2 net) does: the [`ENCOUNTERS`] inputs added with
    /// zero weights.
    #[must_use]
    pub fn with_encounters(&self) -> Self {
        if self.nf != F2 {
            return self.clone();
        }
        let (old, width) = (self.nf + self.nh, F3 + self.nh);
        let mut w = vec![0.0; self.keys * width];
        for key in 0..self.keys {
            let (from, to) = (&self.w[key * old..(key + 1) * old], key * width);
            w[to..to + F2].copy_from_slice(&from[..F2]);
            w[to + F3..to + width].copy_from_slice(&from[F2..]);
        }
        let mut a = vec![0.0; self.nh * F3];
        for (to, from) in a
            .as_chunks_mut::<F3>()
            .0
            .iter_mut()
            .zip(self.a.as_chunks::<F2>().0)
        {
            to[..F2].copy_from_slice(from);
        }
        Self {
            nf: F3,
            w,
            a,
            ..self.clone()
        }
    }

    /// The parameters' count: `w`, `v`, `a`, `b`, `e`.
    const fn len(&self) -> usize {
        self.w.len() + G + self.a.len() + self.b.len() + self.e.len()
    }

    /// The inputs of the keys' rows: the features read, then the hidden units (with the hand's
    /// `hand` keys for a v4 net).
    fn inputs(&self, f: &[f32; F3], hand: &[usize], out: &mut Vec<f32>) {
        out.clear();
        out.extend_from_slice(&f[..self.nf]);
        for (i, (row, b)) in self.a.chunks_exact(self.nf).zip(&self.b).enumerate() {
            let mut pre = b + row
                .iter()
                .zip(&f[..self.nf])
                .map(|(a, x)| a * x)
                .sum::<f32>();
            if !self.e.is_empty() {
                pre += hand
                    .iter()
                    .filter(|&&k| k < self.keys)
                    .map(|&k| self.e[k * self.nh + i])
                    .sum::<f32>();
            }
            out.push(pre.max(0.0));
        }
    }

    fn logit(&self, z: &[f32], key: usize, g: &[f32; G]) -> f32 {
        let width = self.nf + self.nh;
        let row = &self.w[key * width..(key + 1) * width];
        row.iter().zip(z).map(|(w, x)| w * x).sum::<f32>()
            + self.v.iter().zip(g).map(|(v, x)| v * x).sum::<f32>()
    }

    /// The candidates' logits into `out`.
    fn logits(
        &self,
        f: &[f32; F3],
        hand: &[usize],
        cands: &[(usize, [f32; G])],
        out: &mut Vec<f32>,
    ) {
        let mut z = Vec::with_capacity(self.nf + self.nh);
        self.inputs(f, hand, &mut z);
        out.clear();
        out.extend(cands.iter().map(|(k, g)| self.logit(&z, *k, g)));
    }

    /// The policy's move among `legal` (greedy, or sampled with `rng`); `None` when it has no
    /// candidate (a choice is pending, or only potions are left).
    pub fn choose(
        &self,
        combat: &Combat,
        legal: &[Action],
        scratch: &mut Scratch,
        rng: Option<&mut u64>,
    ) -> Option<Action> {
        candidates(combat, legal, &mut scratch.candidates);
        if scratch.candidates.is_empty() {
            return None;
        }
        let f = features(combat, &mut scratch.intents);
        let hand = self.hand_keys(combat);
        self.inputs(&f, &hand, &mut scratch.z);
        scratch.logits.clear();
        for c in &scratch.candidates {
            let l = self.logit(&scratch.z, c.key, &c.g);
            scratch.logits.push(l);
        }
        let i = match rng {
            None => argmax(&scratch.logits),
            Some(rng) => sample(&scratch.logits, rng),
        };
        Some(scratch.candidates[i].action.clone())
    }

    /// The policy's probabilities over `actions` (any legal list) into `out`, at temperature
    /// `temp`: a card play by its key and its own target, ending the turn by its key, and a
    /// potion two logits below the mean of the others.
    pub fn priors(
        &self,
        combat: &Combat,
        actions: &[Action],
        temp: f32,
        scratch: &mut Scratch,
        out: &mut Vec<f32>,
    ) {
        let f = features(combat, &mut scratch.intents);
        let hand = self.hand_keys(combat);
        self.inputs(&f, &hand, &mut scratch.z);
        let inputs = &scratch.z;
        let s = combat.state();
        let total: i32 = s
            .enemies
            .iter()
            .map(|&e| s.creature(e))
            .filter(|c| c.is_alive())
            .map(|c| c.hp + c.block)
            .sum::<i32>()
            .max(1);
        scratch.logits.clear();
        let mut known = Vec::with_capacity(actions.len());
        for action in actions {
            let logit = match action {
                Action::EndTurn => Some(self.logit(inputs, END, &[0.0; G])),
                Action::PlayCard { card, target } => {
                    let c = s.card(*card);
                    let key =
                        usize::from(vocab::card(c.kind)) * 2 + usize::from(c.upgrade_level > 0);
                    let kind = c.kind.card_type();
                    let g = [
                        combat.hooked().energy_cost(*card) as f32 / 3.0,
                        f32::from(kind == CardType::Attack),
                        f32::from(kind == CardType::Skill),
                        f32::from(kind == CardType::Power),
                        f32::from(matches!(kind, CardType::Status | CardType::Curse)),
                        target.map_or(0.0, |t| {
                            (s.creature(t).hp + s.creature(t).block) as f32 / total as f32
                        }),
                    ];
                    Some(self.logit(inputs, key, &g))
                }
                _ => None,
            };
            known.push(logit);
        }
        let n = known.iter().flatten().count().max(1) as f32;
        let mean = known.iter().flatten().sum::<f32>() / n;
        scratch.logits.extend(
            known
                .iter()
                .map(|l| l.unwrap_or(mean - 2.0) / temp.max(1e-3)),
        );
        softmax(&scratch.logits, out);
    }

    /// The hand's keys when the net reads them (v4), else none.
    fn hand_keys(&self, combat: &Combat) -> Vec<usize> {
        if self.e.is_empty() {
            return Vec::new();
        }
        hand_keys(combat)
    }

    /// Whether the net reads the hand (v4).
    #[must_use]
    pub const fn reads_hand(&self) -> bool {
        !self.e.is_empty()
    }

    /// Writes the weights (v1's format without a hidden layer).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let v2 = self.nh > 0 || self.nf != F;
        let magic = if !self.e.is_empty() {
            MAGIC3
        } else if v2 {
            MAGIC2
        } else {
            MAGIC
        };
        let mut out = magic.to_vec();
        let shape: &[usize] = if v2 {
            &[self.keys, self.nf, G, self.nh]
        } else {
            &[self.keys, F, G]
        };
        for &n in shape {
            out.extend(u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes());
        }
        out.extend(vocab::hash().to_le_bytes());
        for x in self
            .w
            .iter()
            .chain(&self.v)
            .chain(&self.a)
            .chain(&self.b)
            .chain(&self.e)
        {
            out.extend(x.to_le_bytes());
        }
        out
    }

    /// Reads weights written by [`RolloutNet::to_bytes`].
    ///
    /// # Errors
    /// If the bytes are malformed or for another vocabulary or shape.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let (magic, rest) = bytes.split_at_checked(8).ok_or("too short")?;
        let dims = match magic {
            m if m == MAGIC => 3,
            m if m == MAGIC2 || m == MAGIC3 => 4,
            _ => return Err("not a rollout policy".to_owned()),
        };
        let (header, rest) = rest.split_at_checked(4 * dims + 8).ok_or("too short")?;
        let u32_at = |i: usize| {
            u32::from_le_bytes([header[i], header[i + 1], header[i + 2], header[i + 3]]) as usize
        };
        let base = Self::new();
        let (nf, nh) = if dims == 4 {
            (u32_at(4), u32_at(12))
        } else {
            (u32_at(4), 0)
        };
        if (u32_at(0), u32_at(8)) != (base.keys, G)
            || !(nf == F || (dims == 4 && (nf == F2 || nf == F3)))
        {
            return Err("another shape".to_owned());
        }
        let mut net = if dims == 4 { base.widened(nh, 0) } else { base };
        net.nf = nf;
        net.w = vec![0.0; net.keys * (nf + nh)];
        net.a = vec![0.0; nh * nf];
        if magic == MAGIC3 {
            net.e = vec![0.0; net.keys * nh];
        }
        let mut hash = [0; 8];
        hash.copy_from_slice(&header[4 * dims..4 * dims + 8]);
        if u64::from_le_bytes(hash) != vocab::hash() {
            return Err("another vocabulary".to_owned());
        }
        let floats: Vec<f32> = rest
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&b| f32::from_le_bytes(b))
            .collect();
        if floats.len() != net.len() {
            return Err("wrong length".to_owned());
        }
        let (w, rest) = floats.split_at(net.w.len());
        let (v, rest) = rest.split_at(G);
        let (a, rest) = rest.split_at(net.a.len());
        let (b, hand) = rest.split_at(net.b.len());
        net.w.copy_from_slice(w);
        net.v.copy_from_slice(v);
        net.a.copy_from_slice(a);
        net.b.copy_from_slice(b);
        net.e.copy_from_slice(hand);
        Ok(net)
    }

    /// Reads the weights at `path`.
    ///
    /// # Errors
    /// If the file is unreadable or not a rollout policy for this build.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Self::from_bytes(&bytes).map_err(|e| format!("{path}: {e}"))
    }
}

impl Default for RolloutNet {
    fn default() -> Self {
        Self::new()
    }
}

fn argmax(xs: &[f32]) -> usize {
    xs.iter()
        .enumerate()
        .fold(0, |b, (i, &x)| if x > xs[b] { i } else { b })
}

/// A step of a 64-bit generator, as a unit float.
fn unit(rng: &mut u64) -> f32 {
    *rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *rng;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1_u64 << 24) as f32
}

/// The softmax probabilities of `logits` into `out`.
fn softmax(logits: &[f32], out: &mut Vec<f32>) {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    out.clear();
    out.extend(logits.iter().map(|l| (l - max).exp()));
    let sum: f32 = out.iter().sum();
    for p in out.iter_mut() {
        *p /= sum;
    }
}

fn sample(logits: &[f32], rng: &mut u64) -> usize {
    let mut p = Vec::with_capacity(logits.len());
    softmax(logits, &mut p);
    let mut u = unit(rng);
    for (i, &q) in p.iter().enumerate() {
        if u < q {
            return i;
        }
        u -= q;
    }
    p.len() - 1
}

/// A training sample of the move `chosen` at `combat` (`None` if it is not a candidate).
#[must_use]
pub fn sample_of(
    combat: &Combat,
    legal: &[Action],
    chosen: &Action,
    scratch: &mut Scratch,
) -> Option<Sample> {
    candidates(combat, legal, &mut scratch.candidates);
    let key = match chosen {
        Action::EndTurn => END,
        Action::PlayCard { card, .. } => {
            let c = combat.state().card(*card);
            usize::from(vocab::card(c.kind)) * 2 + usize::from(c.upgrade_level > 0)
        }
        _ => return None,
    };
    let chosen = scratch.candidates.iter().position(|c| c.key == key)?;
    if scratch.candidates.len() < 2 {
        return None;
    }
    Some(Sample {
        f: features(combat, &mut scratch.intents),
        hand: hand_keys(combat),
        candidates: scratch.candidates.iter().map(|c| (c.key, c.g)).collect(),
        chosen,
    })
}

/// A candidate's key: the card's vocabulary id and whether it is upgraded ([`END`] is 0).
#[must_use]
pub fn card_key(combat: &Combat, card: sts2_core::combat::CardId) -> usize {
    let c = combat.state().card(card);
    usize::from(vocab::card(c.kind)) * 2 + usize::from(c.upgrade_level > 0)
}

/// The keys of the cards in `combat`'s hand.
#[must_use]
pub fn hand_keys(combat: &Combat) -> Vec<usize> {
    combat
        .state()
        .player
        .hand
        .iter()
        .map(|&card| card_key(combat, card))
        .collect()
}

/// What the policy reads at `combat` (RLB17's expert-iteration records): the position's
/// features, its candidates among `legal`, and the keys of the cards in hand.
#[must_use]
pub fn view(
    combat: &Combat,
    legal: &[Action],
    scratch: &mut Scratch,
) -> ([f32; F3], Vec<Candidate>, Vec<usize>) {
    candidates(combat, legal, &mut scratch.candidates);
    let hand = hand_keys(combat);
    (
        features(combat, &mut scratch.intents),
        scratch.candidates.clone(),
        hand,
    )
}

/// Adam over the policy's weights.
#[derive(Clone, Debug)]
pub struct Adam {
    m: Vec<f32>,
    v: Vec<f32>,
    t: i32,
    pub lr: f32,
}

impl Adam {
    #[must_use]
    pub fn new(net: &RolloutNet, lr: f32) -> Self {
        let n = net.len();
        Self {
            m: vec![0.0; n],
            v: vec![0.0; n],
            t: 0,
            lr,
        }
    }

    /// One ascent step along `grad` (the gradient of the objective: `w`, `v`, `a`, then `b`).
    pub fn step(&mut self, net: &mut RolloutNet, grad: &[f32]) {
        self.t += 1;
        let (b1, b2) = (0.9_f32, 0.999_f32);
        let c1 = 1.0 - b1.powi(self.t);
        let c2 = 1.0 - b2.powi(self.t);
        let RolloutNet { w, v, a, b, e, .. } = net;
        let params = w.iter_mut().chain(v.iter_mut()).chain(a).chain(b).chain(e);
        for (i, (p, &g)) in params.zip(grad).enumerate() {
            if g == 0.0 && self.m[i] == 0.0 {
                continue;
            }
            self.m[i] = b1 * self.m[i] + (1.0 - b1) * g;
            self.v[i] = b2 * self.v[i] + (1.0 - b2) * g * g;
            *p += self.lr * (self.m[i] / c1) / ((self.v[i] / c2).sqrt() + 1e-8);
        }
    }
}

/// Adds `scale · ∇ log π(chosen)` for `sample` to `grad`; returns `log π(chosen)`.
fn add_log_grad(net: &RolloutNet, sample: &Sample, scale: f32, grad: &mut [f32]) -> f32 {
    let mut z = Vec::with_capacity(net.nf + net.nh);
    net.inputs(&sample.f, &sample.hand, &mut z);
    let mut logits = Vec::with_capacity(sample.candidates.len());
    logits.extend(sample.candidates.iter().map(|(k, g)| net.logit(&z, *k, g)));
    let mut p = Vec::with_capacity(logits.len());
    softmax(&logits, &mut p);
    let width = net.nf + net.nh;
    let n = net.w.len();
    // The gradient at the hidden units, then back through the layer.
    let mut dh = vec![0.0_f32; net.nh];
    for (j, ((key, g), &pj)) in sample.candidates.iter().zip(&p).enumerate() {
        let d = scale * (f32::from(j == sample.chosen) - pj);
        let at = key * width;
        for (r, x) in grad[at..at + width].iter_mut().zip(&z) {
            *r += d * x;
        }
        for (r, x) in grad[n..n + G].iter_mut().zip(g) {
            *r += d * x;
        }
        for (h, w) in dh.iter_mut().zip(&net.w[at + net.nf..at + width]) {
            *h += d * w;
        }
    }
    let a_at = n + G;
    let b_at = a_at + net.a.len();
    let e_at = b_at + net.b.len();
    for (i, &d) in dh.iter().enumerate() {
        if z[net.nf + i] <= 0.0 || d == 0.0 {
            continue;
        }
        if !net.e.is_empty() {
            for &k in sample.hand.iter().filter(|&&k| k < net.keys) {
                grad[e_at + k * net.nh + i] += d;
            }
        }
        let row = &mut grad[a_at + i * net.nf..a_at + (i + 1) * net.nf];
        for (r, x) in row.iter_mut().zip(&sample.f[..net.nf]) {
            *r += d * x;
        }
        grad[b_at + i] += d;
    }
    p[sample.chosen].max(1e-12).ln()
}

/// Fits `net` to `samples` (cross-entropy, minibatches of 256, `epochs` passes); returns the
/// last epoch's mean log-likelihood and accuracy.
pub fn fit(net: &mut RolloutNet, samples: &[Sample], epochs: usize, lr: f32) -> (f32, f32) {
    let mut adam = Adam::new(net, lr);
    let mut grad = vec![0.0_f32; net.len()];
    let mut order: Vec<usize> = (0..samples.len()).collect();
    let mut rng = 0x5EED_u64;
    let mut stats = (0.0, 0.0);
    for _ in 0..epochs {
        for i in (1..order.len()).rev() {
            let j = (unit(&mut rng) * (i + 1) as f32) as usize;
            order.swap(i, j.min(i));
        }
        let (mut ll, mut hits) = (0.0_f32, 0_usize);
        for batch in order.chunks(256) {
            grad.fill(0.0);
            let scale = 1.0 / batch.len() as f32;
            for &i in batch {
                let s = &samples[i];
                ll += add_log_grad(net, s, scale, &mut grad);
                let mut logits = Vec::new();
                net.logits(&s.f, &s.hand, &s.candidates, &mut logits);
                hits += usize::from(argmax(&logits) == s.chosen);
            }
            adam.step(net, &grad);
        }
        let n = samples.len().max(1) as f32;
        stats = (ll / n, hits as f32 / n);
    }
    stats
}

/// The most actions a training playout takes, and card plays per turn.
const PLAYOUT_ACTIONS: usize = 600;
const PLAYOUT_PLAYS: usize = 25;

/// A playout of `combat` by `net` sampling with `rng`; the moves it chose and its return: a win
/// is `1 + HP/max HP / 2`, a loss `-1` plus up to `0.5` for the enemy HP taken off.
#[must_use]
pub fn sampled_playout(net: &RolloutNet, mut combat: Combat, rng: &mut u64) -> (Vec<Sample>, f32) {
    let mut scratch = Scratch::default();
    let mut legal = Vec::new();
    let mut taken = Vec::new();
    let start = enemy_total(&combat).max(1);
    let mut plays = 0;
    for _ in 0..PLAYOUT_ACTIONS {
        if combat.state().result.is_some() {
            break;
        }
        let action = if combat.pending_decision().is_some() {
            answer_choice(&combat)
        } else {
            combat.legal_actions_into(&mut legal);
            if plays >= PLAYOUT_PLAYS && legal.contains(&Action::EndTurn) {
                Some(Action::EndTurn)
            } else {
                candidates(&combat, &legal, &mut scratch.candidates);
                if scratch.candidates.is_empty() {
                    legal.first().cloned()
                } else {
                    let f = features(&combat, &mut scratch.intents);
                    let hand = net.hand_keys(&combat);
                    let cands: Vec<_> = scratch.candidates.iter().map(|c| (c.key, c.g)).collect();
                    let mut logits = Vec::new();
                    net.logits(&f, &hand, &cands, &mut logits);
                    let i = sample(&logits, rng);
                    let action = scratch.candidates[i].action.clone();
                    if cands.len() > 1 {
                        taken.push(Sample {
                            f,
                            hand,
                            candidates: cands,
                            chosen: i,
                        });
                    }
                    Some(action)
                }
            }
        };
        let Some(action) = action else { break };
        plays = if matches!(action, Action::EndTurn) {
            0
        } else {
            plays + 1
        };
        if combat.apply(action).is_err() {
            break;
        }
    }
    let s = combat.state();
    let me = s.creature(s.player.creature);
    let ret = if s.result == Some(CombatResult::Won) && me.hp > 0 {
        1.0 + 0.5 * me.hp as f32 / me.max_hp.max(1) as f32
    } else {
        let left = enemy_total(&combat).clamp(0, start);
        -1.0 + 0.5 * (start - left) as f32 / start as f32
    };
    (taken, ret)
}

fn enemy_total(combat: &Combat) -> i32 {
    let s = combat.state();
    s.enemies
        .iter()
        .map(|&id| s.creature(id))
        .filter(|c| c.is_alive())
        .map(|c| c.hp + c.block)
        .sum()
}

/// One REINFORCE step on `jobs` threads; returns the mean return and the win rate.
///
/// Each of `starts` gets `per_start` sampled playouts (in worlds blindfolded from `seed`), and a
/// start's mean return is its baseline.
pub fn reinforce(
    net: &mut RolloutNet,
    adam: &mut Adam,
    starts: &[Combat],
    per_start: usize,
    seed: u64,
    jobs: usize,
) -> (f32, f32) {
    let index: Vec<u64> = (0..starts.len() as u64).collect();
    let snapshot: &RolloutNet = net;
    let parts = super::on_threads(&index, jobs, |i| {
        let start = &starts[usize::try_from(i).unwrap_or(0)];
        let mut runs = Vec::with_capacity(per_start);
        for k in 0..per_start {
            let world_seed = seed ^ i.wrapping_mul(0x9E37_79B9) ^ (k as u64) << 40;
            let mut rng = world_seed ^ 0xABCD;
            runs.push(sampled_playout(
                snapshot,
                start.blindfold_for_search(world_seed),
                &mut rng,
            ));
        }
        let base = runs.iter().map(|r| r.1).sum::<f32>() / per_start.max(1) as f32;
        let mut grad = vec![0.0_f32; snapshot.len()];
        let mut wins = 0;
        let mut total = 0.0;
        for (taken, ret) in &runs {
            total += ret;
            wins += usize::from(*ret >= 1.0);
            let adv = ret - base;
            if adv == 0.0 {
                continue;
            }
            for s in taken {
                add_log_grad(snapshot, s, adv, &mut grad);
            }
        }
        (grad, total, wins)
    });
    let mut grad = vec![0.0_f32; net.len()];
    let (mut total, mut wins) = (0.0, 0);
    let episodes = (starts.len() * per_start).max(1) as f32;
    for (g, t, w) in parts {
        for (a, b) in grad.iter_mut().zip(g) {
            *a += b / episodes;
        }
        total += t;
        wins += w;
    }
    adam.step(net, &grad);
    (total / episodes, wins as f32 / episodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The v2 net's gradient of `log π(chosen)` matches finite differences.
    #[test]
    fn the_v2_gradient_matches_finite_differences() {
        let mut net = RolloutNet::new().widened(4, 7);
        let mut rng = 11_u64;
        for x in &mut net.w {
            *x = unit(&mut rng) - 0.5;
        }
        let mut f = [0.0; F3];
        for x in &mut f {
            *x = unit(&mut rng);
        }
        let sample = Sample {
            f,
            hand: Vec::new(),
            candidates: vec![(2, [0.3; G]), (5, [0.1; G]), (END, [0.0; G])],
            chosen: 1,
        };
        let mut grad = vec![0.0; net.len()];
        add_log_grad(&net, &sample, 1.0, &mut grad);
        let log_p = |net: &RolloutNet| {
            let mut logits = Vec::new();
            net.logits(&sample.f, &sample.hand, &sample.candidates, &mut logits);
            let mut p = Vec::new();
            softmax(&logits, &mut p);
            p[sample.chosen].ln()
        };
        let width = F2 + 4;
        // A key row's weight, `v`, a hidden weight and a hidden bias.
        for i in [
            5 * width + 3,
            5 * width + F2 + 1,
            net.w.len() + 2,
            net.w.len() + G + 9,
        ] {
            let eps = 1e-2;
            let mut up = net.clone();
            let mut down = net.clone();
            let n = net.w.len();
            for (m, d) in [(&mut up, eps), (&mut down, -eps)] {
                if i < n {
                    m.w[i] += d;
                } else if i < n + G {
                    m.v[i - n] += d;
                } else {
                    m.a[i - n - G] += d;
                }
            }
            let numeric = (log_p(&up) - log_p(&down)) / (2.0 * eps);
            assert!(
                (numeric - grad[i]).abs() < 2e-3,
                "{i}: {numeric} vs {}",
                grad[i]
            );
        }
    }

    /// The committed v2 weights load as a v2 net.
    #[test]
    fn the_v2_weights_load() {
        let net = RolloutNet::from_bytes(V2).expect("the committed v2 weights load");
        assert_eq!((net.nf, net.nh), (F2, 32));
    }

    /// A v2 net survives its export.
    #[test]
    fn a_v2_net_round_trips() {
        let net = RolloutNet::builtin().widened(8, 3);
        let back = RolloutNet::from_bytes(&net.to_bytes()).expect("a v2 net reads back");
        assert_eq!((back.nf, back.nh, back.a.len()), (F2, 8, 8 * F2));
        assert_eq!(back.w, net.w);
        assert_eq!(back.a, net.a);
    }

    /// A v3 net from a v2 net scores every move as the v2 net does, and survives its export.
    #[test]
    fn the_v4_hand_gradient_matches_finite_differences() {
        let mut net = RolloutNet::new().widened(4, 7);
        let mut rng = 13_u64;
        for x in &mut net.w {
            *x = unit(&mut rng) - 0.5;
        }
        net.e = (0..net.keys * net.nh)
            .map(|_| unit(&mut rng) - 0.3)
            .collect();
        let mut f = [0.0; F3];
        for x in &mut f {
            *x = unit(&mut rng);
        }
        let sample = Sample {
            f,
            hand: vec![2, 5, 5, 9],
            candidates: vec![(2, [0.3; G]), (5, [0.1; G]), (END, [0.0; G])],
            chosen: 0,
        };
        let mut grad = vec![0.0; net.len()];
        add_log_grad(&net, &sample, 1.0, &mut grad);
        let log_p = |net: &RolloutNet| {
            let mut logits = Vec::new();
            net.logits(&sample.f, &sample.hand, &sample.candidates, &mut logits);
            let mut p = Vec::new();
            softmax(&logits, &mut p);
            p[sample.chosen].ln()
        };
        let e_at = net.w.len() + G + net.a.len() + net.b.len();
        for j in [5 * 4 + 1, 5 * 4 + 2, 9 * 4, 2 * 4 + 3] {
            let eps = 1e-2;
            let (mut up, mut down) = (net.clone(), net.clone());
            up.e[j] += eps;
            down.e[j] -= eps;
            let numeric = (log_p(&up) - log_p(&down)) / (2.0 * eps);
            assert!(
                (numeric - grad[e_at + j]).abs() < 2e-3,
                "{j}: {numeric} vs {}",
                grad[e_at + j]
            );
        }
        let back = RolloutNet::from_bytes(&net.to_bytes()).expect("a v4 net reads back");
        assert_eq!(back.e, net.e);
        assert!(back.reads_hand());
    }

    #[test]
    fn a_v3_net_plays_as_its_v2_net() {
        let v2 = RolloutNet::from_bytes(V2).expect("the committed v2 weights load");
        let v3 = v2.with_encounters();
        assert_eq!((v3.nf, v3.nh), (F3, 32));
        let mut rng = 5_u64;
        let mut f = [0.0; F3];
        for x in &mut f {
            *x = unit(&mut rng);
        }
        let cands = [(2, [0.3; G]), (7, [0.1; G]), (END, [0.0; G])];
        let (mut a, mut b) = (Vec::new(), Vec::new());
        v2.logits(&f, &[], &cands, &mut a);
        v3.logits(&f, &[], &cands, &mut b);
        for (x, y) in a.iter().zip(&b) {
            assert!((x - y).abs() < 1e-4, "{x} vs {y}");
        }
        let back = RolloutNet::from_bytes(&v3.to_bytes()).expect("a v3 net reads back");
        assert_eq!(
            (back.nf, back.w.len(), back.a.len()),
            (F3, v3.w.len(), v3.a.len())
        );
    }

    /// Every fight the v3 net tells apart is an encounter of the vocabulary.
    #[test]
    fn the_v3_encounters_are_known() {
        for name in ENCOUNTERS {
            assert_ne!(vocab::encounter_id(name), vocab::UNK, "{name}");
        }
    }
}
