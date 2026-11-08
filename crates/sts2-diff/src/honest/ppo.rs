//! The combat PPO net (`python/train_combat_ppo.py`) as a combat controller.
//!
//! The fight's observation bytes over 255 go through a 2-layer `ReLU` MLP; the action is the
//! masked argmax of the policy head. The weights come from `python/export_combat_policy.py`. The input is the observation the
//! leak test covers, so the net only sees what the player sees.

#![allow(
    clippy::float_arithmetic,
    reason = "the policy net's forward pass; it decides no game rule"
)]

use sts2_core::Combat;
use sts2_core::combat::Action;
use sts2_rl::ACTIONS;
use sts2_rl::action::index_of;
use sts2_rl::obs::{OBS_BYTES, Observer, STATIC_BYTES, observe_static};

/// The export's magic bytes.
const MAGIC: &[u8; 8] = b"STS2PPO1";

/// A dense layer, `out × in` row-major.
#[derive(Clone, Debug)]
struct Layer {
    inputs: usize,
    weights: Vec<f32>,
    bias: Vec<f32>,
}

impl Layer {
    fn forward(&self, x: &[f32], relu: bool) -> Vec<f32> {
        self.bias
            .iter()
            .zip(self.weights.chunks_exact(self.inputs))
            .map(|(&b, row)| {
                let y = b + row.iter().zip(x).map(|(w, v)| w * v).sum::<f32>();
                if relu { y.max(0.0) } else { y }
            })
            .collect()
    }
}

/// The policy part of the combat net.
#[derive(Clone, Debug)]
pub struct Net {
    layers: [Layer; 3],
    /// The first layer's weights by input (`in × out`), so a zero input costs nothing.
    first_by_input: Vec<f32>,
}

impl Net {
    /// Reads the export at `path`, checking its shape and vocabulary against this build.
    ///
    /// # Errors
    /// If the file is unreadable, malformed or for another observation or vocabulary.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&bytes).map_err(|e| format!("{path}: {e}"))
    }

    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let (magic, rest) = bytes.split_at_checked(8).ok_or("too short")?;
        if magic != MAGIC {
            return Err("not a combat policy export".to_owned());
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
        let (obs, hidden, actions) = (u32_at(0), u32_at(4), u32_at(8));
        let mut hash = [0; 8];
        hash.copy_from_slice(&header[12..20]);
        if obs != OBS_BYTES + STATIC_BYTES || actions != ACTIONS {
            return Err(format!(
                "shape {obs}x{actions}, this build has {}x{ACTIONS}",
                OBS_BYTES + STATIC_BYTES
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
        let mut layer = |inputs: usize, outputs: usize| -> Result<Layer, String> {
            Ok(Layer {
                inputs,
                weights: take(inputs * outputs)?,
                bias: take(outputs)?,
            })
        };
        let layers = [
            layer(obs, hidden)?,
            layer(hidden, hidden)?,
            layer(hidden, actions)?,
        ];
        let first = &layers[0];
        let outputs = first.bias.len();
        let mut first_by_input = vec![0.0; first.weights.len()];
        for (o, row) in first.weights.chunks_exact(first.inputs).enumerate() {
            for (i, &w) in row.iter().enumerate() {
                first_by_input[i * outputs + o] = w;
            }
        }
        Ok(Self {
            layers,
            first_by_input,
        })
    }

    /// The greedy legal action in `combat` among `legal` (the fight's actions the run accepts);
    /// `None` if none has an index in the action space.
    #[must_use]
    pub fn choose(&self, combat: &Combat, legal: &[Action]) -> Option<Action> {
        let mut raw = vec![0; OBS_BYTES + STATIC_BYTES];
        let (dynamic, statics) = raw.split_at_mut(OBS_BYTES);
        Observer::new(true).observe(combat, dynamic);
        observe_static(combat, statics);
        let mut h = self.layers[0].bias.clone();
        let outputs = h.len();
        for (i, &b) in raw.iter().enumerate() {
            if b == 0 {
                continue;
            }
            let v = f32::from(b) / 255.0;
            let column = &self.first_by_input[i * outputs..(i + 1) * outputs];
            for (y, w) in h.iter_mut().zip(column) {
                *y += w * v;
            }
        }
        for y in &mut h {
            *y = y.max(0.0);
        }
        let h = self.layers[1].forward(&h, true);
        let logits = self.layers[2].forward(&h, false);
        // The first of equal logits in index order wins, as `torch.argmax`.
        let mut indexed: Vec<(usize, &Action)> = legal
            .iter()
            .filter_map(|a| index_of(combat, a).map(|i| (i, a)))
            .collect();
        indexed.sort_by_key(|(i, _)| *i);
        let mut best: Option<(f32, &Action)> = None;
        for (i, action) in indexed {
            if best.is_none_or(|(l, _)| logits[i] > l) {
                best = Some((logits[i], action));
            }
        }
        best.map(|(_, a)| a.clone())
    }
}
