//! The CB3 combat net in the browser (WEB82): `E2ENet.policy` for one combat row, in f32.
//!
//! It is `python/sts2/e2e/net.py`'s, with the weights `python/web82_export.py` writes (the EMA
//! 0.996 of a CB3 checkpoint; the value module is left out, so nothing privileged is read).
//!
//! [`encode`] turns a run observation into tokens; [`Net::logits`] is the policy's masked
//! logits. A fight's rows never have `MAPNODE` tokens, so the blocks run their plain path.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::as_conversions,
    clippy::many_single_char_names,
    clippy::suboptimal_flops,
    clippy::needless_range_loop,
    reason = "a neural net's forward pass in f32, written as PyTorch computes it (no fused multiply-adds, \
              so the logits match `tests/parity.rs`); the short names and indexed loops are the maths'"
)]

pub mod encode;

use std::collections::HashMap;

use encode::{ACTIONS, ANUM, FC_KINDS, HIST, HNUM, NUM, Tokens, XNUM, ZNUM, kind};
use sts2_rl::entity::ACTION_ENTITIES;

const D: usize = 256;
const HEADS: usize = 4;
const FFN: usize = 704;
const LAYERS: usize = 6;

/// Why weights did not load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadWeights(pub String);

/// A linear map: `w` (out x in, row-major) and an optional bias.
#[derive(Clone, Debug)]
struct Linear {
    w: Vec<f32>,
    b: Option<Vec<f32>>,
    out: usize,
    inp: usize,
}

/// `y = W x (+ b)` of each of `x`'s rows (`inp` wide).
fn dot(a: &[f32], b: &[f32]) -> f32 {
    // Eight accumulators: a vectorisable sum (the order is not PyTorch's, nor is it there).
    let mut acc = [0.0f32; 8];
    let chunks = a.len() / 8;
    for c in 0..chunks {
        let (x, y) = (&a[c * 8..c * 8 + 8], &b[c * 8..c * 8 + 8]);
        for k in 0..8 {
            acc[k] += x[k] * y[k];
        }
    }
    let mut s = ((acc[0] + acc[4]) + (acc[1] + acc[5])) + ((acc[2] + acc[6]) + (acc[3] + acc[7]));
    for k in chunks * 8..a.len() {
        s += a[k] * b[k];
    }
    s
}

impl Linear {
    fn apply(&self, x: &[f32]) -> Vec<f32> {
        let rows = x.len() / self.inp;
        let mut y = vec![0.0; rows * self.out];
        for r in 0..rows {
            let xr = &x[r * self.inp..][..self.inp];
            let yr = &mut y[r * self.out..][..self.out];
            for (o, yo) in yr.iter_mut().enumerate() {
                *yo = dot(&self.w[o * self.inp..][..self.inp], xr)
                    + self.b.as_ref().map_or(0.0, |b| b[o]);
            }
        }
        y
    }
}

/// `Stem`: a linear map, `SiLU`, a linear map.
#[derive(Clone, Debug)]
struct Stem {
    a: Linear,
    b: Linear,
}

impl Stem {
    fn apply(&self, x: &[f32]) -> Vec<f32> {
        let mut h = self.a.apply(x);
        for v in &mut h {
            *v = silu(*v);
        }
        self.b.apply(&h)
    }
}

#[derive(Clone, Debug)]
struct Block {
    n1: Vec<f32>,
    qkv: Linear,
    out: Linear,
    n2: Vec<f32>,
    w1: Linear,
    w3: Linear,
    w2: Linear,
}

fn silu(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

/// `nn.RMSNorm(d)` (eps: f32's machine epsilon) of each row.
fn rms_norm(x: &[f32], w: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    for (xr, yr) in x.chunks(D).zip(y.chunks_mut(D)) {
        let ms = xr.iter().map(|v| v * v).sum::<f32>() / D as f32;
        let k = 1.0 / (ms + f32::EPSILON).sqrt();
        for i in 0..D {
            yr[i] = xr[i] * k * w[i];
        }
    }
    y
}

impl Block {
    fn apply(&self, x: &mut [f32], valid: &[bool]) {
        let t = valid.len();
        let hd = D / HEADS;
        let qkv = self.qkv.apply(&rms_norm(x, &self.n1));
        let scale = 1.0 / (hd as f32).sqrt();
        let mut a = vec![0.0f32; t * D];
        let mut w = vec![0.0f32; t];
        for h in 0..HEADS {
            for i in 0..t {
                let q = &qkv[i * 3 * D + h * hd..][..hd];
                let mut max = f32::NEG_INFINITY;
                for j in 0..t {
                    if valid[j] {
                        let k = &qkv[j * 3 * D + D + h * hd..][..hd];
                        w[j] = dot(q, k) * scale;
                        max = max.max(w[j]);
                    }
                }
                let mut sum = 0.0;
                for j in 0..t {
                    if valid[j] {
                        w[j] = (w[j] - max).exp();
                        sum += w[j];
                    }
                }
                let out = &mut a[i * D + h * hd..][..hd];
                for j in 0..t {
                    if valid[j] {
                        let p = w[j] / sum;
                        let v = &qkv[j * 3 * D + 2 * D + h * hd..][..hd];
                        for (o, vv) in out.iter_mut().zip(v) {
                            *o += p * vv;
                        }
                    }
                }
            }
        }
        for (xv, ov) in x.iter_mut().zip(self.out.apply(&a)) {
            *xv += ov;
        }
        let hn = rms_norm(x, &self.n2);
        let mut g = self.w1.apply(&hn);
        let u = self.w3.apply(&hn);
        for (gv, uv) in g.iter_mut().zip(&u) {
            *gv = silu(*gv) * uv;
        }
        for (xv, ov) in x.iter_mut().zip(self.w2.apply(&g)) {
            *xv += ov;
        }
    }
}

/// The policy's weights.
#[derive(Clone, Debug)]
pub struct Net {
    /// ID-table rows by `id_row` (only the vocabularies a fight reads).
    ids: Vec<f32>,
    stats_base: Vec<f32>,
    stats_up: Vec<f32>,
    /// (first ID row, rows, first compact row) of each shipped vocabulary.
    ranges: Vec<(usize, usize, usize)>,
    up: Vec<f32>,
    kind: Vec<f32>,
    full_kind: Vec<f32>,
    cat: Vec<f32>,
    owner: Vec<f32>,
    stem: Stem,
    hcat: Vec<f32>,
    hmove: Vec<f32>,
    full_stem: Stem,
    link_gain: Vec<f32>,
    fc: Vec<f32>,
    xstem: Stem,
    side: Linear,
    blocks: Vec<Block>,
    norm: Vec<f32>,
    pe: Linear,
    pt: Linear,
    ps: Linear,
    akind: Vec<f32>,
    asub: Vec<f32>,
    plan: Linear,
    pout: Linear,
}

fn row(t: &[f32], i: usize) -> &[f32] {
    &t[i * D..][..D]
}

fn add(x: &mut [f32], y: &[f32]) {
    for (a, b) in x.iter_mut().zip(y) {
        *a += b;
    }
}

struct Reader {
    data: Vec<f32>,
    header: HashMap<String, (usize, Vec<usize>)>,
}

impl Reader {
    fn tensor(&self, name: &str, shape: &[usize]) -> Result<Vec<f32>, BadWeights> {
        let (at, got) = self
            .header
            .get(name)
            .ok_or_else(|| BadWeights(format!("no tensor {name}")))?;
        if got != shape {
            return Err(BadWeights(format!("{name}: shape {got:?}, want {shape:?}")));
        }
        let n: usize = shape.iter().product();
        self.data
            .get(*at..at + n)
            .map(<[f32]>::to_vec)
            .ok_or_else(|| BadWeights(format!("{name}: past the data")))
    }

    fn linear(&self, name: &str, out: usize, inp: usize, bias: bool) -> Result<Linear, BadWeights> {
        Ok(Linear {
            w: self.tensor(&format!("{name}.weight"), &[out, inp])?,
            b: if bias {
                Some(self.tensor(&format!("{name}.bias"), &[out])?)
            } else {
                None
            },
            out,
            inp,
        })
    }

    fn stem(&self, name: &str, inp: usize) -> Result<Stem, BadWeights> {
        Ok(Stem {
            a: self.linear(&format!("{name}.a"), D, inp, true)?,
            b: self.linear(&format!("{name}.b"), D, D, true)?,
        })
    }
}

impl Net {
    /// The net from `python/web82_export.py`'s file.
    ///
    /// # Errors
    /// If the bytes are not such a file of this net's shapes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BadWeights> {
        let bad = |s: &str| BadWeights(s.to_owned());
        if bytes.get(..4) != Some(b"CB3N".as_slice()) {
            return Err(bad("not a CB3N file"));
        }
        let len = usize::try_from(u32::from_le_bytes(
            bytes
                .get(4..8)
                .ok_or_else(|| bad("short"))?
                .try_into()
                .map_err(|_| bad("short"))?,
        ))
        .map_err(|_| bad("header length"))?;
        let head: serde_json::Value =
            serde_json::from_slice(bytes.get(8..8 + len).ok_or_else(|| bad("short header"))?)
                .map_err(|e| BadWeights(e.to_string()))?;
        let mut header = HashMap::new();
        let mut ranges = Vec::new();
        for (k, v) in head.as_object().ok_or_else(|| bad("header"))? {
            if k == "_id_ranges" {
                let mut compact = 0;
                for r in v.as_array().ok_or_else(|| bad("ranges"))? {
                    let s = r[0].as_u64().and_then(|x| usize::try_from(x).ok());
                    let n = r[1].as_u64().and_then(|x| usize::try_from(x).ok());
                    let (Some(s), Some(n)) = (s, n) else {
                        return Err(bad("range"));
                    };
                    ranges.push((s, n, compact));
                    compact += n;
                }
            } else if !k.starts_with('_') {
                let at = v[0].as_u64().and_then(|x| usize::try_from(x).ok());
                let shape: Option<Vec<usize>> = v[1].as_array().and_then(|a| {
                    a.iter()
                        .map(|x| x.as_u64().and_then(|x| usize::try_from(x).ok()))
                        .collect()
                });
                let (Some(at), Some(shape)) = (at, shape) else {
                    return Err(BadWeights(format!("entry {k}")));
                };
                header.insert(k.clone(), (at, shape));
            }
        }
        let data = bytes[8 + len..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        let r = Reader { data, header };
        let rows: usize = ranges.iter().map(|x| x.1).sum();
        let blocks = (0..LAYERS)
            .map(|i| {
                let p = format!("blocks.{i}");
                Ok(Block {
                    n1: r.tensor(&format!("{p}.n1.weight"), &[D])?,
                    qkv: r.linear(&format!("{p}.qkv"), 3 * D, D, false)?,
                    out: r.linear(&format!("{p}.out"), D, D, false)?,
                    n2: r.tensor(&format!("{p}.n2.weight"), &[D])?,
                    w1: r.linear(&format!("{p}.w1"), FFN, D, false)?,
                    w3: r.linear(&format!("{p}.w3"), FFN, D, false)?,
                    w2: r.linear(&format!("{p}.w2"), D, FFN, false)?,
                })
            })
            .collect::<Result<Vec<_>, BadWeights>>()?;
        Ok(Self {
            ids: r.tensor("ids", &[rows, D])?,
            stats_base: r.tensor("stats_base", &[rows, D])?,
            stats_up: r.tensor("stats_up", &[rows, D])?,
            ranges,
            up: r.tensor("up", &[D])?,
            kind: r.tensor("kind.weight", &[kind::BASE_KINDS, D])?,
            full_kind: r.tensor("full.kind.weight", &[2, D])?,
            cat: r.tensor("cat.weight", &[64, D])?,
            owner: r.tensor("owner.weight", &[32, D])?,
            stem: r.stem("stem", NUM)?,
            hcat: r.tensor("full.hcat.weight", &[320, D])?,
            hmove: r.tensor("full.hmove.weight", &[1 + HIST * encode::MOVE_BUCKETS, D])?,
            full_stem: r.stem("full.stem", HNUM)?,
            link_gain: r.tensor("x_v2.link_gain", &[D])?,
            fc: r.tensor("x_v2.fc.weight", &[3 * FC_KINDS, D])?,
            xstem: r.stem("x_v2.stem", XNUM)?,
            side: r.linear("x_side.proj", D, ZNUM, false)?,
            blocks,
            norm: r.tensor("norm.weight", &[D])?,
            pe: r.linear("pe", D, D, true)?,
            pt: r.linear("pt", D, D, false)?,
            ps: r.linear("ps", D, D, false)?,
            akind: r.tensor("akind.weight", &[32, D])?,
            asub: r.tensor("asub.weight", &[16, D])?,
            plan: r.linear("x_plan.proj", D, ANUM, false)?,
            pout: r.linear("pout", 1, D, true)?,
        })
    }

    /// The compact row of ID-table row `id` (row 0 of the table when it was not shipped).
    fn id_row(&self, id: usize) -> usize {
        self.ranges
            .iter()
            .find(|&&(s, n, _)| (s..s + n).contains(&id))
            .map_or(0, |&(s, _, c)| c + id - s)
    }

    /// The trunk's output (T x D) of a row's tokens.
    fn trunk(&self, tok: &Tokens) -> Vec<f32> {
        let t = tok.tokens.len();
        let mut x = vec![0.0f32; t * D];
        let nums: Vec<f32> = tok.tokens.iter().flat_map(|k| k.num).collect();
        let stem = self.stem.apply(&nums);
        let hnums: Vec<f32> = tok.tokens.iter().flat_map(|k| k.hnum).collect();
        let full_stem = self.full_stem.apply(&hnums);
        let xnums: Vec<f32> = tok.tokens.iter().flat_map(|k| k.xnum).collect();
        let xstem = self.xstem.apply(&xnums);
        let znums: Vec<f32> = tok.tokens.iter().flat_map(|k| k.znum).collect();
        let side = self.side.apply(&znums);
        for (i, k) in tok.tokens.iter().enumerate() {
            let xr = &mut x[i * D..][..D];
            let id = self.id_row(k.ident);
            let kind_row = if k.kind < kind::BASE_KINDS {
                row(&self.kind, k.kind)
            } else {
                row(&self.full_kind, k.kind - kind::BASE_KINDS)
            };
            for j in 0..D {
                xr[j] = (((row(&self.ids, id)[j] + kind_row[j])
                    + row(&self.cat, k.cat.min(63))[j])
                    + row(&self.owner, k.owner.min(31))[j])
                    + stem[i * D + j];
            }
            // The full view's extras: the slots' sum (index 0 adds nothing), then the stem.
            let mut slots = [0.0f32; D];
            for &c in k.hcat.iter().filter(|&&c| c > 0) {
                add(&mut slots, row(&self.hcat, c));
            }
            let mut moves = [0.0f32; D];
            for &m in k.hmove.iter().filter(|&&m| m > 0) {
                add(&mut moves, row(&self.hmove, m));
            }
            for j in 0..D {
                xr[j] += (slots[j] + moves[j]) + full_stem[i * D + j];
            }
            let stats = if k.up() {
                &self.stats_up
            } else {
                &self.stats_base
            };
            add(xr, row(stats, id));
        }
        // Pile summaries: the mean of each group's cards at positions 1-4.
        let mut summ = [[0.0f32; D]; 4];
        let mut count = [0usize; 4];
        for &(card, up, g) in &tok.bag {
            let id = self.id_row(card);
            let stats = if up > 0.0 {
                &self.stats_up
            } else {
                &self.stats_base
            };
            for j in 0..D {
                summ[g - 1][j] += (row(&self.ids, id)[j] + up * self.up[j]) + row(stats, id)[j];
            }
            count[g - 1] += 1;
        }
        for g in 0..4 {
            let n = count[g].max(1) as f32;
            for j in 0..D {
                x[(g + 1) * D + j] += summ[g][j] / n;
            }
        }
        // v2: the intents' links, the forecasts, the v2 stem, the side columns.
        for (i, k) in tok.tokens.iter().enumerate() {
            let xr = &mut x[i * D..][..D];
            if k.link > 0 {
                let o = row(&self.owner, k.link.min(31));
                for j in 0..D {
                    xr[j] += self.link_gain[j] * o[j];
                }
            }
            for j in 0..D {
                let fc = (row(&self.fc, k.xcat[0])[j] + row(&self.fc, k.xcat[1])[j])
                    + row(&self.fc, k.xcat[2])[j];
                xr[j] = ((xr[j] + fc) + xstem[i * D + j]) + side[i * D + j];
            }
        }
        let valid: Vec<bool> = tok.tokens.iter().map(|k| k.valid).collect();
        for blk in &self.blocks {
            blk.apply(&mut x, &valid);
        }
        rms_norm(&x, &self.norm)
    }

    /// The policy's logits over the run space ([`ACTIONS`]), `-inf` off the mask.
    #[must_use]
    pub fn logits(&self, tok: &Tokens) -> Vec<f32> {
        let h = self.trunk(tok);
        let t = tok.tokens.len();
        let ps = self.ps.apply(row(&h, 0));
        let mut out = vec![f32::NEG_INFINITY; ACTIONS];
        for a in (0..ACTIONS).filter(|&a| tok.legal[a]) {
            let e = ACTION_ENTITIES[a];
            let pe = self.pe.apply(row(&h, tok.ent[a].min(t - 1)));
            let mut z: Vec<f32> = pe.iter().zip(&ps).map(|(p, s)| p + s).collect();
            if let Some(g) = tok.tgt[a] {
                add(&mut z, &self.pt.apply(row(&h, g.min(t - 1))));
            }
            let (ak, asub) = (
                row(&self.akind, e.kind as usize),
                row(&self.asub, usize::from(e.sub).min(15)),
            );
            for j in 0..D {
                z[j] = (z[j] + ak[j]) + asub[j];
            }
            let anum = tok.anum.get(a).copied().unwrap_or([0.0; ANUM]);
            add(&mut z, &self.plan.apply(&anum));
            for v in &mut z {
                *v = silu(*v);
            }
            out[a] = self.pout.apply(&z)[0];
        }
        out
    }

    /// The legal index with the highest logit (the first of equals), if any.
    #[must_use]
    pub fn greedy(logits: &[f32]) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, &l) in logits.iter().enumerate() {
            if l > f32::NEG_INFINITY && best.is_none_or(|b| l > logits[b]) {
                best = Some(i);
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::encode::symlog;

    #[test]
    fn symlog_is_torchs() {
        assert!((symlog(9.0) - 10.0f32.ln()).abs() < 1e-6);
        assert!((symlog(-9.0) + 10.0f32.ln()).abs() < 1e-6);
        assert_eq!(symlog(0.0).abs(), 0.0);
    }
}
