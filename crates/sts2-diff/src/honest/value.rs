//! The combat value/policy net (RLB11) for the search: the win probability and HP kept of a
//! position, and a move prior.
//!
//! It is trained on the GPU by `python/train_combat_puffer.py` (masked PPO over real fight
//! starts of the bot's runs) and exported by `python/sts2/combat_net.py`, whose `features` this
//! module mirrors exactly: the input is the observation the leak test covers (the dynamic part
//! and the static part of `sts2_rl::obs`), so the net only sees what the player sees.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "the net's forward pass; it decides no game rule"
)]

use sts2_core::Combat;
use sts2_core::combat::Action;
use sts2_rl::ACTIONS;
use sts2_rl::action::index_of;
use sts2_rl::obs::{
    CARD_BYTES, CARDS, CARDS_AT, CHOICE_AT, CREATURE_BYTES, CREATURES, CREATURES_AT, DECK, HAND,
    HAND_AT, INTENT_BYTES, INTENTS, INTENTS_AT, OBS_BYTES, OPTIONS, Observer, POTIONS, POWERS,
    RELICS, STATIC_BYTES, observe_static,
};

/// The export's magic bytes.
const MAGIC: &[u8; 8] = b"STS2VAL1";
/// The committed weights.
static BUILTIN: &[u8] = include_bytes!("../../data/value.bin");
/// VN1's candidate, refit to the real bot's fight outcomes (`--value vn1`; not the default).
static VN1: &[u8] = include_bytes!("../../data/value-vn1.bin");

/// The embedding tables in export order: rows and width.
const TABLES: [(usize, usize); 8] = [
    (1024, 32), // card (kind × 2 + upgraded)
    (256, 16),  // monster
    (512, 16),  // power kind
    (512, 16),  // power amount
    (32, 8),    // intent kind + 1
    (512, 16),  // relic
    (128, 16),  // potion
    (128, 16),  // encounter
];
const CARD: usize = 0;
const MONSTER: usize = 1;
const POWER_KIND: usize = 2;
const POWER_AMOUNT: usize = 3;
const INTENT: usize = 4;
const RELIC: usize = 5;
const POTION: usize = 6;
const ENCOUNTER: usize = 7;

/// Features per creature, hand slot, intent and choice option.
const PER_CREATURE: usize = 6 + 16 + 16 + 16;
const PER_HAND: usize = 32 + 9;
const PER_INTENT: usize = 8 + 4;
const PER_OPTION: usize = 32 + 1;
/// The input width.
pub const FEATURES: usize = 17
    + CREATURES * PER_CREATURE
    + 4 * 32
    + HAND * PER_HAND
    + INTENTS * PER_INTENT
    + OPTIONS * PER_OPTION
    + 3
    + 8
    + 16
    + 16
    + POTIONS * 16;

/// The static part's offsets within the whole input.
const RELICS_AT: usize = OBS_BYTES + 6 + DECK * 3;
const POTIONS_AT: usize = RELICS_AT + RELICS * 2;

/// The net's reading of a position.
#[derive(Clone, Debug)]
pub struct Output {
    /// Policy logits over the action space.
    pub logits: Vec<f32>,
    /// The probability of winning the fight.
    pub win: f32,
    /// The expected HP fraction kept (0 for a loss).
    pub hp: f32,
}

/// A dense layer stored input-major (`in × out`), so the forward pass is a sum of scaled
/// columns.
#[derive(Clone, Debug)]
struct Layer {
    outputs: usize,
    by_input: Vec<f32>,
    bias: Vec<f32>,
}

impl Layer {
    fn forward(&self, x: &[f32], relu: bool) -> Vec<f32> {
        let mut y = self.bias.clone();
        for (&v, column) in x.iter().zip(self.by_input.chunks_exact(self.outputs)) {
            if v == 0.0 {
                continue;
            }
            for (y, w) in y.iter_mut().zip(column) {
                *y += w * v;
            }
        }
        if relu {
            for y in &mut y {
                *y = y.max(0.0);
            }
        }
        y
    }

    /// [`Layer::forward`] on each of `xs` (rows of the input width), each weight column read
    /// once for all of them; every output is summed in the same order as alone.
    fn forward_batch(&self, xs: &[Vec<f32>], relu: bool) -> Vec<Vec<f32>> {
        let mut ys: Vec<Vec<f32>> = xs.iter().map(|_| self.bias.clone()).collect();
        for (i, column) in self.by_input.chunks_exact(self.outputs).enumerate() {
            for (x, y) in xs.iter().zip(&mut ys) {
                let v = x[i];
                if v == 0.0 {
                    continue;
                }
                for (y, w) in y.iter_mut().zip(column) {
                    *y += w * v;
                }
            }
        }
        if relu {
            for y in ys.iter_mut().flatten() {
                *y = y.max(0.0);
            }
        }
        ys
    }
}

/// The value/policy net.
#[derive(Clone, Debug)]
pub struct ValueNet {
    tables: Vec<Vec<f32>>,
    l1: Layer,
    l2: Layer,
    policy: Layer,
    win: Layer,
    hp: Layer,
}

/// Little-endian readers over the raw observation.
fn u16_at(raw: &[u8], at: usize) -> usize {
    usize::from(raw[at]) | usize::from(raw[at + 1]) << 8
}

fn i16_at(raw: &[u8], at: usize) -> f32 {
    f32::from(i16::from_le_bytes([raw[at], raw[at + 1]]))
}

impl ValueNet {
    /// Reads the export at `path`, checking its shape and vocabulary against this build.
    ///
    /// # Errors
    /// If the file is unreadable, malformed or for another input or vocabulary.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&bytes).map_err(|e| format!("{path}: {e}"))
    }

    /// The committed weights (`data/value.bin`, provenance in `data/value.md`).
    ///
    /// # Errors
    /// If they were exported for another input or vocabulary (retrain after a change there).
    pub fn builtin() -> Result<Self, String> {
        Self::parse(BUILTIN).map_err(|e| format!("data/value.bin: {e}"))
    }

    /// VN1's weights (`data/value-vn1.bin`, provenance in `data/value-vn1.md`).
    ///
    /// # Errors
    /// As [`ValueNet::builtin`].
    pub fn vn1() -> Result<Self, String> {
        Self::parse(VN1).map_err(|e| format!("data/value-vn1.bin: {e}"))
    }

    /// The net `name` names on the command line: `builtin`, `vn1` or an export's path.
    ///
    /// # Errors
    /// As [`ValueNet::load`].
    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "builtin" => Self::builtin(),
            "vn1" => Self::vn1(),
            path => Self::load(path),
        }
    }

    /// The export in `bytes`.
    ///
    /// # Errors
    /// As [`ValueNet::load`].
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let (magic, rest) = bytes.split_at_checked(8).ok_or("too short")?;
        if magic != MAGIC {
            return Err("not a combat value net export".to_owned());
        }
        let (header, mut rest) = rest.split_at_checked(20).ok_or("too short")?;
        let u32_at = |i: usize| {
            usize::try_from(u32::from_le_bytes([
                header[i],
                header[i + 1],
                header[i + 2],
                header[i + 3],
            ]))
            .unwrap_or(usize::MAX)
        };
        let (features, hidden, actions) = (u32_at(0), u32_at(4), u32_at(8));
        let mut hash = [0; 8];
        hash.copy_from_slice(&header[12..20]);
        if features != FEATURES || actions != ACTIONS {
            return Err(format!(
                "shape {features}x{actions}, this build has {FEATURES}x{ACTIONS}"
            ));
        }
        if u64::from_le_bytes(hash) != sts2_rl::vocab::hash() {
            return Err("trained on another vocabulary".to_owned());
        }
        let mut take = |n: usize| -> Result<Vec<f32>, String> {
            let (head, tail) = rest.split_at_checked(n * 4).ok_or("truncated")?;
            rest = tail;
            Ok(head
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&b| f32::from_le_bytes(b))
                .collect())
        };
        let tables = TABLES
            .iter()
            .map(|&(rows, width)| take(rows * width))
            .collect::<Result<Vec<_>, _>>()?;
        let mut layer = |inputs: usize, outputs: usize| -> Result<Layer, String> {
            let weights = take(inputs * outputs)?;
            let bias = take(outputs)?;
            let mut by_input = vec![0.0; weights.len()];
            for (o, row) in weights.chunks_exact(inputs).enumerate() {
                for (i, &w) in row.iter().enumerate() {
                    by_input[i * outputs + o] = w;
                }
            }
            Ok(Layer {
                outputs,
                by_input,
                bias,
            })
        };
        let l1 = layer(FEATURES, hidden)?;
        let l2 = layer(hidden, hidden)?;
        let policy = layer(hidden, ACTIONS)?;
        let win = layer(hidden, 1)?;
        let hp = layer(hidden, 1)?;
        if !rest.is_empty() {
            return Err("trailing bytes".to_owned());
        }
        Ok(Self {
            tables,
            l1,
            l2,
            policy,
            win,
            hp,
        })
    }

    /// Row `id` of table `t` (ids past the last row share it).
    fn row(&self, t: usize, id: usize) -> &[f32] {
        let (rows, width) = TABLES[t];
        let id = id.min(rows - 1);
        &self.tables[t][id * width..(id + 1) * width]
    }

    /// The features of a raw observation (the dynamic part, then the static part).
    #[must_use]
    pub fn features(&self, raw: &[u8]) -> Vec<f32> {
        let mut x = Vec::with_capacity(FEATURES);
        Self::header(raw, &mut x);
        self.creatures(raw, &mut x);
        self.cards(raw, &mut x);
        self.rest(raw, &mut x);
        debug_assert_eq!(x.len(), FEATURES);
        x
    }

    /// The header's scalars.
    fn header(raw: &[u8], x: &mut Vec<f32>) {
        let hp = i16_at(raw, 0);
        let max_hp = i16_at(raw, 2);
        let status = raw[22];
        x.extend([
            hp / 100.0,
            max_hp / 100.0,
            hp / max_hp.max(1.0),
            i16_at(raw, 4) / 50.0,
            i16_at(raw, 6) / 5.0,
            i16_at(raw, 8) / 5.0,
            i16_at(raw, 10) / 10.0,
            u16_at(raw, 12).min(30) as f32 / 10.0,
            u16_at(raw, 14).min(30) as f32 / 10.0,
            f32::from(raw[16]) / 10.0,
            f32::from(raw[17]) / 30.0,
            f32::from(raw[18]) / 30.0,
            f32::from(raw[19]) / 30.0,
            f32::from(raw[20]) / 5.0,
            f32::from(raw[21]) / 5.0,
            f32::from(u8::from(status == 0)),
            f32::from(u8::from(status == 1)),
        ]);
    }

    /// Each creature's scalars, monster and powers.
    fn creatures(&self, raw: &[u8], x: &mut Vec<f32>) {
        for c in 0..CREATURES {
            let at = CREATURES_AT + c * CREATURE_BYTES;
            let (c_hp, c_max) = (i16_at(raw, at + 6), i16_at(raw, at + 8));
            x.extend([
                f32::from(raw[at]),
                f32::from(raw[at + 1]),
                c_hp / 100.0,
                c_max / 100.0,
                c_hp / c_max.max(1.0),
                i16_at(raw, at + 10) / 50.0,
            ]);
            x.extend_from_slice(self.row(MONSTER, u16_at(raw, at + 4)));
            let mut kinds = [0.0_f32; 16];
            let mut amounts = [0.0_f32; 16];
            for p in 0..POWERS {
                let kind = u16_at(raw, at + 12 + 4 * p);
                if kind == 0 {
                    continue;
                }
                let amount = i16_at(raw, at + 14 + 4 * p).clamp(-50.0, 50.0) / 10.0;
                for (k, w) in kinds.iter_mut().zip(self.row(POWER_KIND, kind)) {
                    *k += w;
                }
                for (a, w) in amounts.iter_mut().zip(self.row(POWER_AMOUNT, kind)) {
                    *a += w * amount;
                }
            }
            x.extend(kinds);
            x.extend(amounts);
        }
    }

    /// The card piles pooled and the hand by slot.
    fn cards(&self, raw: &[u8], x: &mut Vec<f32>) {
        let card_token = |i: usize| {
            let at = CARDS_AT + i * CARD_BYTES;
            u16_at(raw, at) * 2 + usize::from(raw[at + 2].min(1))
        };
        for pile in 1..=4 {
            let mut sum = [0.0_f32; 32];
            for i in 0..CARDS {
                let at = CARDS_AT + i * CARD_BYTES;
                if u16_at(raw, at) == 0 || raw[at + 3] != pile {
                    continue;
                }
                for (s, w) in sum.iter_mut().zip(self.row(CARD, card_token(i))) {
                    *s += w / 8.0;
                }
            }
            x.extend(sum);
        }
        let hand_n = usize::from(raw[16]);
        for slot in 0..HAND {
            let id = u16_at(raw, HAND_AT + 2 * slot);
            if slot >= hand_n || id >= CARDS {
                x.extend([0.0; PER_HAND]);
                continue;
            }
            let at = CARDS_AT + id * CARD_BYTES;
            x.extend_from_slice(self.row(CARD, card_token(id)));
            x.push(f32::from(i8::from_le_bytes([raw[at + 4]])) / 5.0);
            x.push(f32::from(raw[at + 2].min(1)));
            x.extend((0..6).map(|k| f32::from((raw[at + 5] >> k) & 1)));
            x.push(1.0);
        }
    }

    /// The intents, a pending choice and the static part.
    fn rest(&self, raw: &[u8], x: &mut Vec<f32>) {
        let card_token = |i: usize| {
            let at = CARDS_AT + i * CARD_BYTES;
            u16_at(raw, at) * 2 + usize::from(raw[at + 2].min(1))
        };
        for i in 0..INTENTS {
            let at = INTENTS_AT + i * INTENT_BYTES;
            x.extend_from_slice(self.row(INTENT, usize::from(raw[at])));
            let dmg = i16_at(raw, at + 1);
            let hits = f32::from(raw[at + 3]);
            x.extend([
                dmg / 20.0,
                hits / 5.0,
                dmg * hits / 50.0,
                f32::from(raw[at + 4]) / 5.0,
            ]);
        }
        let choice_max = raw[CHOICE_AT + OPTIONS * 2 + 1];
        for slot in 0..OPTIONS {
            let id = u16_at(raw, CHOICE_AT + 2 * slot);
            if (id == 0 && slot > 0) || choice_max == 0 || id >= CARDS {
                x.extend([0.0; PER_OPTION]);
                continue;
            }
            x.extend_from_slice(self.row(CARD, card_token(id)));
            x.push(1.0);
        }
        x.extend((0..3).map(|k| f32::from(raw[CHOICE_AT + OPTIONS * 2 + k]) / 5.0));
        let act = usize::from(raw[OBS_BYTES + 2].min(3));
        let room = usize::from(raw[OBS_BYTES + 3].min(3));
        x.extend((0..4).map(|i| f32::from(u8::from(i == act))));
        x.extend((0..4).map(|i| f32::from(u8::from(i == room))));
        x.extend_from_slice(self.row(ENCOUNTER, u16_at(raw, OBS_BYTES + 4)));
        let mut relics = [0.0_f32; 16];
        for r in 0..RELICS {
            let id = u16_at(raw, RELICS_AT + 2 * r);
            if id == 0 {
                continue;
            }
            for (s, w) in relics.iter_mut().zip(self.row(RELIC, id)) {
                *s += w / 4.0;
            }
        }
        x.extend(relics);
        for p in 0..POTIONS {
            x.extend_from_slice(self.row(POTION, u16_at(raw, POTIONS_AT + 2 * p)));
        }
    }

    /// The net on a raw observation.
    #[must_use]
    pub fn forward(&self, raw: &[u8]) -> Output {
        let h = self.l1.forward(&self.features(raw), true);
        let h = self.l2.forward(&h, true);
        let sigmoid = |v: f32| 1.0 / (1.0 + (-v).exp());
        Output {
            logits: self.policy.forward(&h, false),
            win: sigmoid(self.win.forward(&h, false)[0]),
            hp: sigmoid(self.hp.forward(&h, false)[0]),
        }
    }

    /// The net on each of `combats` in one batch: [`ValueNet::eval`] of each, exactly.
    #[must_use]
    pub fn eval_batch(&self, combats: &[&Combat]) -> Vec<Output> {
        if combats.is_empty() {
            return Vec::new();
        }
        let xs: Vec<Vec<f32>> = combats
            .iter()
            .map(|c| self.features(&observation(c)))
            .collect();
        let h = self.l1.forward_batch(&xs, true);
        let h = self.l2.forward_batch(&h, true);
        let sigmoid = |v: f32| 1.0 / (1.0 + (-v).exp());
        let logits = self.policy.forward_batch(&h, false);
        let win = self.win.forward_batch(&h, false);
        let hp = self.hp.forward_batch(&h, false);
        logits
            .into_iter()
            .zip(win.into_iter().zip(hp))
            .map(|(logits, (w, h))| Output {
                logits,
                win: sigmoid(w[0]),
                hp: sigmoid(h[0]),
            })
            .collect()
    }

    /// The net on `combat`'s observation.
    #[must_use]
    pub fn eval(&self, combat: &Combat) -> Output {
        self.forward(&observation(combat))
    }

    /// `combat`'s value on a scale where a win is `win`: `win` × (P(win) + `hp` / 1000 × the
    /// HP fraction kept). The HP kept is capped at the HP the player has: past that the net's
    /// error would make a fight left going worth more than one won now.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a search score from the net's probabilities"
    )]
    pub fn score(&self, combat: &Combat, win: i64, hp: i64) -> i64 {
        let out = self.eval(combat);
        let s = combat.state();
        let me = s.creature(s.player.creature);
        let now = me.hp.max(0) as f32 / me.max_hp.max(1) as f32;
        (win as f32 * (out.win + out.hp.min(now) * hp as f32 / 1000.0)) as i64
    }

    /// The policy's log-probability of each of `actions` among them, in thousandths of a
    /// nat (an action without an index gets the least of the others).
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a search score from the net's probabilities"
    )]
    pub fn log_prior(&self, combat: &Combat, actions: &[Action]) -> Vec<i64> {
        let out = self.eval(combat);
        let logits: Vec<Option<f32>> = actions
            .iter()
            .map(|a| index_of(combat, a).map(|i| out.logits[i]))
            .collect();
        let top = logits.iter().flatten().copied().fold(f32::MIN, f32::max);
        let sum: f32 = logits.iter().flatten().map(|l| (l - top).exp()).sum();
        let logp: Vec<Option<f32>> = logits
            .iter()
            .map(|l| l.map(|l| l - top - sum.ln()))
            .collect();
        let least = logp.iter().flatten().copied().fold(0.0, f32::min);
        logp.iter()
            .map(|l| (l.unwrap_or(least) * 1000.0) as i64)
            .collect()
    }

    /// The policy's probabilities of `actions` among them from `out` (the net on `combat`), at
    /// temperature `temp`; an action without an index gets the least logit of the others.
    #[must_use]
    pub fn priors(out: &Output, combat: &Combat, actions: &[Action], temp: f32) -> Vec<f32> {
        let logits: Vec<Option<f32>> = actions
            .iter()
            .map(|a| index_of(combat, a).map(|i| out.logits[i] / temp))
            .collect();
        let least = logits.iter().flatten().copied().fold(f32::MAX, f32::min);
        let top = logits.iter().flatten().copied().fold(f32::MIN, f32::max);
        let exp: Vec<f32> = logits
            .iter()
            .map(|l| (l.unwrap_or(least) - top).exp())
            .collect();
        let sum: f32 = exp.iter().sum();
        exp.iter().map(|e| e / sum.max(f32::MIN_POSITIVE)).collect()
    }

    /// The greedy legal action among `legal` (the first of equal logits in index order);
    /// `None` if none has an index in the action space.
    #[must_use]
    pub fn choose(&self, combat: &Combat, legal: &[Action]) -> Option<Action> {
        let out = self.eval(combat);
        let mut indexed: Vec<(usize, &Action)> = legal
            .iter()
            .filter_map(|a| index_of(combat, a).map(|i| (i, a)))
            .collect();
        indexed.sort_by_key(|(i, _)| *i);
        let mut best: Option<(f32, &Action)> = None;
        for (i, action) in indexed {
            if best.is_none_or(|(l, _)| out.logits[i] > l) {
                best = Some((out.logits[i], action));
            }
        }
        best.map(|(_, a)| a.clone())
    }
}

/// `combat`'s observation: the dynamic part, then the static part.
#[must_use]
pub fn observation(combat: &Combat) -> Vec<u8> {
    let mut raw = vec![0; OBS_BYTES + STATIC_BYTES];
    let (dynamic, statics) = raw.split_at_mut(OBS_BYTES);
    Observer::new(true).observe(combat, dynamic);
    observe_static(combat, statics);
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks `net` against a fixture of `python/rlb11_fixture.py`; the largest error.
    fn parity(net: &ValueNet, fixture: &[u8]) -> f32 {
        let width = OBS_BYTES + STATIC_BYTES;
        let count = u32::from_le_bytes(fixture[..4].try_into().expect("4 bytes"));
        let f32s = |b: &[u8]| -> Vec<f32> {
            b.as_chunks::<4>()
                .0
                .iter()
                .map(|&c| f32::from_le_bytes(c))
                .collect()
        };
        let row = width + 4 * (2 + ACTIONS);
        let mut worst = 0.0_f32;
        for r in fixture[4..].chunks_exact(row).take(count as usize) {
            let (raw, rest) = r.split_at(width);
            let expected = f32s(rest);
            let out = net.forward(raw);
            worst = worst
                .max((out.win - expected[0]).abs())
                .max((out.hp - expected[1]).abs());
            for (a, b) in out.logits.iter().zip(&expected[2..]) {
                worst = worst.max((a - b).abs());
            }
        }
        worst
    }

    /// The batched forward pass gives each position's single one exactly.
    #[test]
    fn a_batch_matches_single_positions() {
        let net = ValueNet::builtin().expect("the committed value net loads");
        let combats: Vec<Combat> = (0..5).map(|s| sts2_rl::Start::w1_all(s).combat).collect();
        let refs: Vec<&Combat> = combats.iter().collect();
        for (c, out) in combats.iter().zip(net.eval_batch(&refs)) {
            let one = net.eval(c);
            assert_eq!((one.win, one.hp), (out.win, out.hp));
            assert_eq!(one.logits, out.logits);
        }
    }

    /// The committed weights give torch's outputs on the committed fixture
    /// (`python/rlb11_fixture.py`, 64 positions of random play on held-out boss starts).
    #[test]
    fn the_builtin_net_matches_pytorch() {
        let net = ValueNet::builtin().expect("the committed value net loads");
        let worst = parity(&net, include_bytes!("../../data/value-check.bin"));
        assert!(worst < 1e-3, "largest difference {worst}");
    }

    /// VN1's weights load in this build and score positions as probabilities.
    #[test]
    fn the_vn1_net_loads() {
        let net = ValueNet::vn1().expect("VN1's value net loads");
        for s in 0..3 {
            let out = net.eval(&sts2_rl::Start::w1_all(s).combat);
            assert!((0.0..=1.0).contains(&out.win) && (0.0..=1.0).contains(&out.hp));
        }
    }
}
