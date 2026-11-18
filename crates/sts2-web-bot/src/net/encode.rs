//! A combat row's tokens, as the CB3 net reads them.
//!
//! This is `python/sts2/e2e/encode.py`'s `encode(obs, mask, v2=True, full=True, discard=True)`
//! for one row in a fight (phase 3), ported line by line. Outside a fight every
//! section but the run's (bosses, the room, the belt, the relics) is empty, so only the fight's
//! sections are built; [`encode`] refuses other phases.
//!
//! The numbers are the Python's: the same columns in the same order, `symlog` in f32, the
//! `knapsack` in integers. `tests/parity.rs` checks the whole path against the logits of `PyTorch`.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::as_conversions,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "the encoder's f32 columns, computed as the Python does (game numbers far below \
              f32's exact range); the short names and indexed loops follow `encode.py` line by line"
)]

use sts2_rl::entity::{ACTION_ENTITIES, EntityKind};
use sts2_rl::run_action::RUN_ACTIONS;

/// Numeric columns per token (`NUM`).
pub const NUM: usize = 48;
/// v2 numeric columns per token (`XNUM`).
pub const XNUM: usize = 16;
/// RLN21 side columns per token (`ZNUM`).
pub const ZNUM: usize = 4;
/// Plan columns per action (`ANUM`).
pub const ANUM: usize = 5;
/// Past moves per creature (`HIST`).
pub const HIST: usize = 3;
/// The past moves' numeric columns (`HNUM`).
pub const HNUM: usize = 5 * HIST + 1;
/// Forecast kind rows per step (`FC_KINDS`).
pub const FC_KINDS: usize = 32;
/// Move-key buckets (`MOVE_BUCKETS`).
pub const MOVE_BUCKETS: usize = 1024;
/// The token width with the full view (`T_MAX_FULL`).
pub const T_MAX: usize = 128;
/// STATE and the four pile summaries.
pub const FIXED: usize = 5;
/// The run space with the discard-potion indices (`ACTIONS_ALL`).
pub const ACTIONS: usize = RUN_ACTIONS;
/// The combat indices the plan column covers (`COMBAT_ACTIONS`).
pub const COMBAT_ACTIONS: usize = sts2_rl::action::ACTIONS;

/// Token kinds (`encode.KINDS` then `FULL_KINDS`), the ones a fight uses.
pub mod kind {
    pub const STATE: usize = 0;
    pub const BOSS: usize = 5;
    pub const PAGE: usize = 6;
    pub const HAND: usize = 8;
    pub const CREATURE: usize = 9;
    pub const POWER: usize = 10;
    pub const INTENT: usize = 11;
    pub const CHOICE: usize = 12;
    pub const BELT: usize = 13;
    pub const RELIC: usize = 14;
    /// The first full-view kind (`pile`); the net's `kind` table has the 29 before it.
    pub const PILE: usize = 29;
    /// Kinds of `encode.KINDS`.
    pub const BASE_KINDS: usize = 29;
}

/// The ID table's offset of each vocabulary (`encode._OFF`).
pub mod off {
    pub const CARDS: usize = 0;
    pub const RELICS: usize = 1024;
    pub const MONSTERS: usize = 1536;
    pub const POWERS: usize = 1792;
    pub const POTIONS: usize = 2304;
    pub const ENCOUNTERS: usize = 2432;
    pub const EVENTS: usize = 2688;
    /// Rows of the ID table.
    pub const ID_ROWS: usize = 4096;
}

// Run observation offsets (`run_obs.rs`), as `encode.py` has them.
const RELICS_AT: usize = 352;
const BELT_AT: usize = 480;
const SELECT_AT: usize = 1192;
const COMBAT_AT: usize = 1750;
const V2_AT: usize = 2996;
const FULL_AT: usize = 4116;
/// The bytes a row needs (`OBS_BYTES_FULL`).
pub const OBS_BYTES: usize = 4340;
const C_CREATURES: usize = 24;
const C_CARDS: usize = 504;
const C_HAND: usize = 888;
const C_INTENTS: usize = 908;
const C_CHOICE: usize = 948;
const CREATURE_BYTES: usize = 60;
const V_LINK: usize = 0;
const V_FORECAST: usize = 8;
const V_PREVIEW: usize = 176;
const V_AIMED: usize = 236;
const V_SIDE: usize = 396;
const V_DISPLAY: usize = 412;
const V_PLAN: usize = 700;
const STEPS: usize = 3;
const HIST_SLOT: usize = 28;
const HIST_MOVE: usize = 9;
const PICK_BASE: usize = sts2_rl::action::PICK_BASE;
const PHASE_COMBAT: u8 = 3;
const ENERGY_CAP: usize = 15;
const POWER_DOOM: i32 = 59;
const POWER_DIE_FOR_YOU: i32 = 57;

const _: () = {
    use sts2_rl::run_obs as R;
    assert!(R::RELICS_AT == RELICS_AT && R::BELT_AT == BELT_AT && R::SELECT_AT == SELECT_AT);
    assert!(R::COMBAT_AT == COMBAT_AT && R::V2_AT == V2_AT && R::FULL_AT == FULL_AT);
    assert!(R::FULL_AT + R::FULL_BYTES == OBS_BYTES);
    assert!(sts2_rl::obs_v2::PLAN_AT == V_PLAN && sts2_rl::obs_v2::SIDE_AT == V_SIDE);
    assert!(sts2_rl::obs_v2::DISPLAY_AT == V_DISPLAY && sts2_rl::obs_v2::AIMED_AT == V_AIMED);
    assert!(sts2_rl::obs_v2::PREVIEW_AT == V_PREVIEW && sts2_rl::obs_v2::FORECAST_AT == V_FORECAST);
    assert!(ACTIONS == 361 && COMBAT_ACTIONS == 140 && PICK_BASE == 129);
};

/// One token's inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// In the attention (`valid`): the fixed summaries of empty piles are not.
    pub valid: bool,
    pub kind: usize,
    /// A row of the ID table.
    pub ident: usize,
    pub cat: usize,
    pub owner: usize,
    pub num: [f32; NUM],
    pub xnum: [f32; XNUM],
    pub xcat: [usize; STEPS],
    pub link: usize,
    pub znum: [f32; ZNUM],
    /// The past moves' `hcat` slots (a creature's first `HIST`; 0 adds nothing).
    pub hcat: [usize; HIST],
    pub hmove: [usize; HIST],
    pub hnum: [f32; HNUM],
}

impl Token {
    const fn new(kind: usize) -> Self {
        Self {
            valid: true,
            kind,
            ident: 0,
            cat: 0,
            owner: 0,
            num: [0.0; NUM],
            xnum: [0.0; XNUM],
            xcat: [0; STEPS],
            link: 0,
            znum: [0.0; ZNUM],
            hcat: [0; HIST],
            hmove: [0; HIST],
            hnum: [0.0; HNUM],
        }
    }

    /// Whether the token reads its card's upgraded content-stats row (`token_up`).
    #[must_use]
    pub fn up(&self) -> bool {
        matches!(self.kind, kind::HAND | kind::CHOICE | kind::PILE) && self.num[0] > 0.5
    }
}

/// A row's tokens and its action inputs.
#[derive(Clone, Debug)]
pub struct Tokens {
    pub tokens: Vec<Token>,
    /// The pile summaries' cards: (ID row, upgrade, group 1-4).
    pub bag: Vec<(usize, f32, usize)>,
    pub legal: Vec<bool>,
    /// Per index, its entity's and target's token positions (`ent`, `tgt`; -1 none).
    pub ent: Vec<usize>,
    pub tgt: Vec<Option<usize>>,
    /// The plan column of the combat indices.
    pub anum: Vec<[f32; ANUM]>,
}

/// `symlog` in f32.
#[must_use]
pub fn symlog(x: f32) -> f32 {
    x.signum() * x.abs().ln_1p() * f32::from(u8::from(x != 0.0))
}

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&self, at: usize) -> i32 {
        i32::from(self.0[at])
    }
    fn i8(&self, at: usize) -> i32 {
        i32::from(i8::from_le_bytes([self.0[at]]))
    }
    fn u16(&self, at: usize) -> i32 {
        i32::from(u16::from_le_bytes([self.0[at], self.0[at + 1]]))
    }
    fn i16(&self, at: usize) -> i32 {
        i32::from(i16::from_le_bytes([self.0[at], self.0[at + 1]]))
    }
}

const fn f(x: i32) -> f32 {
    x as f32
}

fn b(x: bool) -> f32 {
    f32::from(u8::from(x))
}

fn ix(x: i32) -> usize {
    usize::try_from(x.max(0)).unwrap_or(0)
}

/// A card table entry's (ok, ID row, num columns) (`card_of`).
fn card_of(r: &Bytes<'_>, id: i32, valid: bool) -> (bool, usize, [f32; 9]) {
    let i = ix(id.clamp(0, 63));
    let at = COMBAT_AT + C_CARDS + 6 * i;
    let (kind, up, cost, flags) = (r.u16(at), r.u8(at + 2), r.i8(at + 4), r.u8(at + 5));
    let mut num = [f(up), f(cost), b(cost < 0), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    for k in 0..6 {
        num[3 + k] = f((flags >> k) & 1);
    }
    (valid && id < 64, off::CARDS + ix(kind), num)
}

/// Why a row has no tokens here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAFight {
    /// The row is narrower than [`OBS_BYTES`].
    Short,
    /// The pending decision is not a fight's.
    Phase(u8),
}

/// A candidate before compaction: its token and, for entity lookups, (section kind, index).
struct Cand {
    token: Token,
    section: usize,
    index: usize,
}

/// The tokens of a combat row: `obs` (a run observation with the v2 and full-view sections) and
/// `mask` (the env's mask words).
///
/// # Errors
/// If the row is not a fight's or is too short.
#[expect(
    clippy::too_many_lines,
    reason = "a straight port of encode.py's sections, in order"
)]
pub fn encode(obs: &[u8], mask: &[u64]) -> Result<Tokens, NotAFight> {
    if obs.len() < OBS_BYTES {
        return Err(NotAFight::Short);
    }
    if obs[3] != PHASE_COMBAT {
        return Err(NotAFight::Phase(obs[3]));
    }
    let r = Bytes(obs);
    let legal: Vec<bool> = (0..ACTIONS)
        .map(|i| mask.get(i / 64).is_some_and(|w| w >> (i % 64) & 1 == 1))
        .collect();
    let mut cands: Vec<Cand> = Vec::new();
    let mut push = |token: Token, section: usize, index: usize| {
        if token.valid {
            cands.push(Cand {
                token,
                section,
                index,
            });
        }
    };

    // Bosses, the room.
    for s in 0..2 {
        let boss = r.u16(20 + 2 * s);
        let mut t = Token::new(kind::BOSS);
        t.valid = boss > 0;
        t.ident = off::ENCOUNTERS + ix(boss);
        t.cat = s;
        push(t, kind::BOSS, s);
    }
    let room_type = r.u8(26);
    let model = r.u16(28);
    let mut t = Token::new(kind::PAGE);
    t.valid = model > 0;
    t.ident = if room_type > 0 {
        off::ENCOUNTERS
    } else {
        off::EVENTS
    } + ix(model);
    t.cat = ix(room_type);
    push(t, kind::PAGE, 0);

    // The creatures' numbers, used by several sections.
    let cb = COMBAT_AT + C_CREATURES;
    let creature = |c: usize, o: usize| cb + CREATURE_BYTES * c + o;
    let alive: Vec<i32> = (0..8).map(|c| r.u8(creature(c, 0))).collect();
    let side: Vec<i32> = (0..8).map(|c| r.u8(creature(c, 1))).collect();
    let hp: Vec<i32> = (0..8).map(|c| r.i16(creature(c, 6))).collect();
    let max_hp: Vec<i32> = (0..8).map(|c| r.i16(creature(c, 8))).collect();
    let block: Vec<i32> = (0..8).map(|c| r.i16(creature(c, 10))).collect();
    let creature_ok: Vec<bool> = max_hp.iter().map(|&m| m > 0).collect();
    let pkind = |c: usize, j: usize| r.u16(creature(c, 12 + 4 * j));
    let pamt = |c: usize, j: usize| r.i16(creature(c, 14 + 4 * j));

    // v2: the hand's playability and the knapsack (`_v2`).
    let v = V2_AT;
    let mut playable = [false; 10];
    for (i, e) in ACTION_ENTITIES.iter().enumerate() {
        if e.kind == EntityKind::HandCard && legal[i] {
            playable[usize::from(e.slot).min(9)] = true;
        }
    }
    let energy = r.i16(COMBAT_AT + 6);
    let mut kcost = [0i32; 10];
    let mut value = [[0i32; 8]; 10];
    for i in 0..10 {
        let id = ix(r.u16(COMBAT_AT + C_HAND + 2 * i).clamp(0, 63));
        let cost = r.i8(COMBAT_AT + C_CARDS + 6 * id + 4);
        let k = if cost < 0 { energy } else { cost };
        kcost[i] = if playable[i] {
            k
        } else {
            i32::try_from(ENERGY_CAP).unwrap_or(15) + 1
        };
        let ph = r.u8(v + V_PREVIEW + 6 * i + 4);
        for (t, val) in value[i].iter_mut().enumerate() {
            *val = r.i16(v + V_AIMED + 2 * (8 * i + t)) * ph * i32::from(playable[i]);
        }
    }
    let most = knapsack(&value, &kcost, energy);
    let need: Vec<i32> = (0..8).map(|c| hp[c] + block[c]).collect();
    let killable: Vec<bool> = (0..8)
        .map(|c| alive[c] > 0 && side[c] == 1 && need[c] > 0 && most[c] >= need[c])
        .collect();
    let doomed: Vec<bool> = (0..8)
        .map(|c| {
            let doom: i32 = (0..12)
                .map(|j| pamt(c, j) * i32::from(pkind(c, j) == POWER_DOOM))
                .sum();
            let after = hp[c] - (most[c] - block[c]).max(0);
            alive[c] > 0 && side[c] == 1 && doom > 0 && after <= doom
        })
        .collect();

    // Hand.
    let hand_size = ix(r.u8(COMBAT_AT + 16));
    for i in 0..10 {
        let id = r.u16(COMBAT_AT + C_HAND + 2 * i);
        let (ok, ident, num) = card_of(&r, id, i < hand_size);
        let mut t = Token::new(kind::HAND);
        t.valid = ok;
        t.ident = ident;
        t.num[..9].copy_from_slice(&num);
        t.num[9] = f(i32::try_from(i).unwrap_or(0));
        let at = v + V_PREVIEW + 6 * i;
        let (pd, pb, ph, pf) = (r.i16(at), r.i16(at + 2), r.u8(at + 4), r.u8(at + 5));
        t.xnum[..7].copy_from_slice(&[
            symlog(f(pd)),
            symlog(f(pb)),
            f(ph),
            f(pf & 1),
            f((pf >> 1) & 1),
            symlog(f(pd * ph)),
            b(playable[i]),
        ]);
        push(t, kind::HAND, i);
    }

    // Creatures.
    for c in 0..8 {
        let mut t = Token::new(kind::CREATURE);
        t.valid = creature_ok[c];
        t.ident = off::MONSTERS + ix(r.u16(creature(c, 4)));
        t.cat = ix(side[c] * 2 + i32::from(c == 0 && side[c] == 0));
        t.owner = c + 1;
        t.num[..5].copy_from_slice(&[
            f(alive[c]),
            symlog(f(hp[c])),
            symlog(f(max_hp[c])),
            symlog(f(block[c])),
            f(hp[c]) / f(max_hp[c].max(1)),
        ]);
        for k in 0..STEPS {
            let at = v + V_FORECAST + 7 * (c * STEPS + k);
            t.xcat[k] = ix(r.u8(at).clamp(0, 31)) + k * FC_KINDS;
            t.xnum[k] = f(r.u8(at + 1)) / 255.0;
            t.xnum[3 + k] = symlog(f(r.i16(at + 2)));
            t.xnum[6 + k] = f(r.u8(at + 4));
            t.xnum[9 + k] = symlog(f(r.i16(at + 5)));
        }
        t.xnum[12] = symlog(f(most[c]));
        t.xnum[13] = b(killable[c]);
        t.xnum[14] = (f(most[c]) / f(need[c].max(1))).min(2.0);
        t.xnum[15] = b(doomed[c]);
        let facing = r.u8(v + V_SIDE + 2 * c);
        let arm = r.u8(v + V_SIDE + 2 * c + 1);
        t.znum = [b(facing == 1), b(facing == 2), b(arm == 1), b(arm == 2)];
        // The full view's past moves (`_full_extras`).
        let base = FULL_AT + HIST_SLOT * c;
        for k in 0..HIST {
            let at = base + 1 + HIST_MOVE * k;
            let key = r.u16(at);
            let present = key > 0;
            if present {
                t.hcat[k] = k * FC_KINDS + ix(r.u8(at + 3) + 1).min(FC_KINDS - 1);
                t.hmove[k] = 1 + k * MOVE_BUCKETS + ix(key) % MOVE_BUCKETS;
            }
            t.hnum[5 * k..5 * k + 5].copy_from_slice(&[
                b(present),
                f(r.u8(at + 2)),
                symlog(f(r.i16(at + 4))),
                f(r.u8(at + 6)),
                symlog(f(r.i16(at + 7))),
            ]);
        }
        t.hnum[HNUM - 1] = symlog(f(r.u8(base)));
        push(t, kind::CREATURE, c);
    }

    // Powers.
    for c in 0..8 {
        for j in 0..12 {
            let (pk, pa) = (pkind(c, j), pamt(c, j));
            let mut t = Token::new(kind::POWER);
            t.valid = pk > 0 && creature_ok[c];
            t.ident = off::POWERS + ix(pk);
            t.owner = c + 1;
            t.num[..2].copy_from_slice(&[symlog(f(pa)), b(pa < 0)]);
            let at = v + V_DISPLAY + 3 * (12 * c + j);
            let shown = r.u8(at);
            t.znum[..2].copy_from_slice(&[f(shown), symlog(f(r.i16(at + 1) * shown))]);
            push(t, kind::POWER, 12 * c + j);
        }
    }

    // Intents.
    let mut incoming = 0;
    for s in 0..8 {
        let at = COMBAT_AT + C_INTENTS + 5 * s;
        let (ik, dmg, hits, stat) = (r.u8(at), r.i16(at + 1), r.u8(at + 3), r.u8(at + 4));
        incoming += dmg * hits;
        let mut t = Token::new(kind::INTENT);
        t.valid = ik > 0;
        t.cat = ix(ik);
        t.num[..5].copy_from_slice(&[
            symlog(f(dmg)),
            f(hits),
            symlog(f(dmg * hits)),
            f(stat),
            f(i32::try_from(s).unwrap_or(0)),
        ]);
        t.link = ix(r.u8(v + V_LINK + s));
        push(t, kind::INTENT, s);
    }

    // Combat choice options.
    for k in 0..10 {
        let id = r.u16(COMBAT_AT + C_CHOICE + 2 * k);
        let (ok, ident, num) = card_of(&r, id, legal[PICK_BASE + k]);
        let mut t = Token::new(kind::CHOICE);
        t.valid = ok;
        t.ident = ident;
        t.num[..9].copy_from_slice(&num);
        push(t, kind::CHOICE, k);
    }

    // Belt, relics.
    for s in 0..6 {
        let potion = r.u16(BELT_AT + 2 * s);
        let mut t = Token::new(kind::BELT);
        t.valid = potion > 0;
        t.ident = off::POTIONS + ix(potion);
        t.num[0] = f(i32::try_from(s).unwrap_or(0));
        push(t, kind::BELT, s);
    }
    for s in 0..32 {
        let relic = r.u16(RELICS_AT + 4 * s);
        let flags = r.u8(RELICS_AT + 4 * s + 2);
        let mut t = Token::new(kind::RELIC);
        t.valid = relic > 0;
        t.ident = off::RELICS + ix(relic);
        t.num[..2].copy_from_slice(&[f(flags & 1), f((flags >> 1) & 1)]);
        push(t, kind::RELIC, s);
    }

    // The pile cards (`_pile_section`): one token per distinct card, sorted by its bytes.
    let mut piles: Vec<(i64, usize)> = (0..64)
        .filter_map(|i| {
            let at = COMBAT_AT + C_CARDS + 6 * i;
            let (k, up, pile, cost, flags) = (
                r.u16(at),
                r.u8(at + 2),
                r.u8(at + 3),
                r.i8(at + 4),
                r.u8(at + 5),
            );
            (k > 0 && (2..=4).contains(&pile)).then(|| {
                let key = ((i64::from(k) * 256 + i64::from(up.clamp(0, 255))) * 256
                    + i64::from(cost + 128))
                    * 64
                    + i64::from(flags);
                (key, i)
            })
        })
        .collect();
    piles.sort_by_key(|&(key, i)| (key, i));
    let mut groups: Vec<(i64, usize, [i32; 3])> = Vec::new();
    for &(key, i) in &piles {
        let pile = ix(r.u8(COMBAT_AT + C_CARDS + 6 * i + 3) - 2).min(2);
        match groups.last_mut() {
            Some(g) if g.0 == key => g.2[pile] += 1,
            _ => {
                let mut counts = [0; 3];
                counts[pile] = 1;
                groups.push((key, i, counts));
            }
        }
    }
    for (n, &(_, i, counts)) in groups.iter().enumerate() {
        let (_, ident, num) = card_of(&r, i32::try_from(i).unwrap_or(0), true);
        let mut t = Token::new(kind::PILE);
        t.ident = ident;
        t.cat = (0..3).map(|k| usize::from(counts[k] > 0) << k).sum();
        t.num[..9].copy_from_slice(&num);
        for k in 0..3 {
            t.num[9 + k] = symlog(f(counts[k]));
        }
        push(t, kind::PILE, n);
    }

    // Compaction from FIXED, then the fixed positions.
    let keep = cands.len().min(T_MAX - FIXED);
    cands.truncate(keep);
    let mut tokens: Vec<Token> = Vec::with_capacity(FIXED + keep);
    let mut state = Token::new(kind::STATE);
    state.cat = usize::from(PHASE_COMBAT);
    state.num = state_num(&r);
    let absorb: i32 = (1..8)
        .filter(|&c| {
            alive[c] > 0 && side[c] == 0 && (0..12).any(|j| pkind(c, j) == POWER_DIE_FOR_YOU)
        })
        .map(|c| (hp[c] + block[c]).max(0))
        .sum();
    let lost = (incoming - r.i16(COMBAT_AT + 4) - absorb).max(0);
    state.xnum[..6].copy_from_slice(&[
        symlog(f(lost)),
        b(lost >= r.i16(COMBAT_AT) && lost > 0),
        b(killable.iter().any(|&k| k)),
        symlog(f(incoming)),
        b(doomed.iter().any(|&d| d)),
        symlog(f(absorb)),
    ]);
    tokens.push(state);
    let mut bag = Vec::new();
    let mut counts = [0i32; 4];
    for i in 0..64 {
        let at = COMBAT_AT + C_CARDS + 6 * i;
        let (k, up, pile) = (r.u16(at), r.u8(at + 2), r.u8(at + 3));
        if k > 0 && (2..=4).contains(&pile) {
            let g = ix(pile - 1);
            counts[g - 1] += 1;
            bag.push((off::CARDS + ix(k), f(up), g));
        }
    }
    for (g, &n) in counts.iter().enumerate() {
        let mut t = Token::new(g + 1);
        t.valid = n > 0;
        t.num[0] = symlog(f(n));
        t.num[1] = b(n > 0);
        tokens.push(t);
    }
    let pos = |section: usize, index: usize| {
        cands
            .iter()
            .position(|c| c.section == section && c.index == index)
            .map_or(0, |p| p + FIXED)
    };
    let mut ent = vec![0; ACTIONS];
    let mut tgt = vec![None; ACTIONS];
    // The k-th living enemy's token (sub k + 1).
    let enemies: Vec<usize> = (0..8)
        .filter(|&c| creature_ok[c] && side[c] == 1)
        .map(|c| pos(kind::CREATURE, c))
        .collect();
    for (i, e) in ACTION_ENTITIES.iter().enumerate() {
        let slot = usize::from(e.slot);
        ent[i] = match e.kind {
            EntityKind::HandCard => pos(kind::HAND, slot),
            EntityKind::CombatPotion | EntityKind::BeltPotion | EntityKind::DiscardPotion => {
                pos(kind::BELT, slot)
            }
            EntityKind::ChoiceOption => pos(kind::CHOICE, slot),
            _ => 0,
        };
        if matches!(e.kind, EntityKind::HandCard | EntityKind::CombatPotion) && e.sub >= 1 {
            tgt[i] = enemies.get(usize::from(e.sub - 1).min(6)).copied();
        }
    }
    tokens.extend(cands.into_iter().map(|c| c.token));
    Ok(Tokens {
        tokens,
        bag,
        legal,
        ent,
        tgt,
        anum: plan_columns(&r),
    })
}

/// Per creature slot, the most of `value` the playable cards fit in `energy` (`_knapsack`).
fn knapsack(value: &[[i32; 8]; 10], cost: &[i32; 10], energy: i32) -> [i32; 8] {
    let cap = ENERGY_CAP + 1;
    let mut dp = [[0i32; ENERGY_CAP + 1]; 8];
    let mut free = [0i32; 8];
    for i in 0..10 {
        let k = cost[i];
        let kk = ix(k.clamp(1, i32::try_from(cap).unwrap_or(16)));
        for t in 0..8 {
            let v = value[i][t];
            if k == 0 {
                free[t] += v;
            }
            let old = dp[t];
            for c in 0..cap {
                if c >= kk && k > 0 {
                    dp[t][c] = old[c].max(old[c - kk] + v);
                }
            }
        }
    }
    let at = ix(energy.clamp(0, i32::try_from(ENERGY_CAP).unwrap_or(15)));
    let mut most = [0; 8];
    for t in 0..8 {
        most[t] = dp[t][at] + free[t];
    }
    most
}

/// The STATE token's numbers (`_state_num`).
fn state_num(r: &Bytes<'_>) -> [f32; NUM] {
    let c = COMBAT_AT;
    let s = SELECT_AT;
    let not255 = |x: i32| x * i32::from(x != 255);
    let h = [
        f(r.u8(2)),
        f(r.u8(6)),
        f(r.u8(7)),
        symlog(f(r.i16(8))),
        symlog(f(r.i16(10))),
        f(r.i16(8)) / f(r.i16(10).max(1)),
        symlog(f(r.i16(12))),
        f(r.u8(14)),
        f(r.u8(15)),
        symlog(f(r.u16(16))),
        f(r.u8(18)),
        f(r.u8(19)),
        f(not255(r.u8(24))),
        f(not255(r.u8(25))),
        symlog(f(r.i16(c))),
        symlog(f(r.i16(c + 2))),
        symlog(f(r.i16(c + 4))),
        f(r.i16(c + 6)),
        f(r.i16(c + 8)),
        f(r.i16(c + 10)),
        symlog(f(r.u16(c + 12))),
        f(r.u8(c + 16)),
        symlog(f(r.u8(c + 17))),
        symlog(f(r.u8(c + 18))),
        symlog(f(r.u8(c + 19))),
        f(r.u8(c + 20)),
        f(r.u8(c + 21)),
        f(r.u8(c + 22)),
        f(r.u8(c + C_CHOICE + 20)),
        f(r.u8(c + C_CHOICE + 21)),
        f(r.u8(c + C_CHOICE + 22)),
        f(r.u8(s + 1)),
        f(r.u8(s + 2)),
        f(r.u8(s + 3)),
        f(r.u8(s + 4)),
    ];
    let mut out = [0.0; NUM];
    out[..h.len()].copy_from_slice(&h);
    // A one-hot of the selection purpose (12 columns), then the room type's (one column).
    let purpose = r.u8(s);
    if purpose > 0 {
        out[h.len() + ix(purpose.clamp(0, 11))] = 1.0;
    }
    out[NUM - 1] = 1.0;
    out
}

/// The plan column of the combat indices (`plan_columns`).
fn plan_columns(r: &Bytes<'_>) -> Vec<[f32; ANUM]> {
    let at = |a: usize| V2_AT + V_PLAN + 3 * a;
    let best = (0..COMBAT_ACTIONS)
        .filter(|&a| r.u8(at(a) + 2) & 1 == 1)
        .map(|a| r.i16(at(a)))
        .max()
        .unwrap_or(-(1 << 20));
    (0..COMBAT_ACTIONS)
        .map(|a| {
            let flags = r.u8(at(a) + 2);
            if flags & 1 == 0 {
                return [0.0; ANUM];
            }
            let score = r.i16(at(a));
            [
                1.0,
                f((flags >> 1) & 1),
                f((flags >> 2) & 1),
                symlog(f(score) / 10.0),
                symlog(f(score - best) / 10.0),
            ]
        })
        .collect()
}
