//! The learned run policy (RLB8, `docs/specs/run-policy.md`): every run decision outside a
//! fight, scored by a net over the run observation.
//!
//! [`Features::of`] reads the run observation's bytes ([`sts2_rl::run_obs`], covered by the
//! observation leak test) and the legal list, and nothing else: the deck, relics and belt as
//! token bags, about forty scalars, and one candidate per legal run-level index with its tokens
//! (the action kind and what it offers) and scalars (price, copies in the deck, a map node's
//! forward cone). [`RunNet`] scores every candidate against the state; the policy is the
//! softmax over them. The weights come from `python/train_run_ppo.py`.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "the policy net's features and forward pass; they decide no game rule"
)]

use std::sync::OnceLock;

use sts2_core::content::cards::{CardKind, CardType};
use sts2_core::run::{RunAction, RunDecision, RunState};
use sts2_rl::run_action::{ALTERNATIVE_BASE, ALTERNATIVE_SLOTS, REWARD_CARD_SLOTS};
use sts2_rl::run_action::{
    ANCIENT_BASE, BUNDLE_BASE, BUY_CARD_BASE, BUY_POTION_BASE, BUY_RELIC_BASE, CANCEL_SELECT,
    CLAIM_BASE, CONFIRM_SELECT, EVENT_BASE, NODE_BASE, PICK_CARD_BASE, PROCEED, REMOVE_CARD,
    REST_BASE, SELECT_BASE, SKIP_BASE, TAKE_RELIC, USE_POTION_BASE, run_level_index,
};
use sts2_rl::run_obs::{
    ANCIENT_AT, BELT, BELT_AT, BUNDLE_CARDS, BUNDLES_AT, DECK, DECK_AT, EVENT_AT, MAP_AT,
    MAP_POINT_BYTES, MAP_POINTS, RELICS, RELICS_AT, REST_AT, REWARD_BYTES, REWARDS_AT,
    RUN_OBS_BYTES, RunObserver, SELECT_AT, SELECT_OPTION_BYTES, SHOP_AT, SHOP_CARDS, SHOP_POTIONS,
    SHOP_RELICS, TREASURE_AT,
};
use sts2_rl::vocab;

/// The feature layout's version; an export for another version does not load.
pub const FEATURE_VERSION: u32 = 1;

/// Token blocks of the embedding table, each `(offset, size)`; ids past a block's size are
/// clamped to its `UNK` (1).
const CARD_SLOTS: usize = 1024;
mod tok {
    pub const CARD: usize = 1; // card id × 2 + upgraded
    pub const ENCHANT: usize = CARD + 2 * super::CARD_SLOTS;
    pub const RELIC: usize = ENCHANT + 32;
    pub const POTION: usize = RELIC + 512;
    pub const ENCOUNTER: usize = POTION + 128;
    pub const EVENT: usize = ENCOUNTER + 128;
    pub const ANCIENT: usize = EVENT + 128;
    pub const TEXT: usize = ANCIENT + 32;
    pub const TEXT_BUCKETS: usize = 2048;
    pub const ROOM: usize = TEXT + TEXT_BUCKETS;
    pub const REST: usize = ROOM + 16;
    pub const REWARD: usize = REST + 16;
    pub const PURPOSE: usize = REWARD + 8;
    pub const KIND: usize = PURPOSE + 32;
    pub const PHASE: usize = KIND + 32;
    pub const ACT: usize = PHASE + 16;
    pub const TOKENS: usize = ACT + 8;
}
/// Rows of the embedding table.
pub const TOKENS: usize = tok::TOKENS;

/// State token slots: the deck's cards, then their enchantments, the relics, the belt and the
/// context (boss, phase, act, the screen's own: event, ancient or selection purpose).
pub const DECK_TOKENS: usize = 2 * DECK;
pub const RELIC_TOKENS: usize = RELICS;
pub const BELT_TOKENS: usize = 8;
pub const CONTEXT_TOKENS: usize = 4;
pub const STATE_TOKENS: usize = DECK_TOKENS + RELIC_TOKENS + BELT_TOKENS + CONTEXT_TOKENS;
/// State scalars.
pub const STATE_SCALARS: usize = 40;
/// Tokens and scalars per candidate.
pub const CAND_TOKENS: usize = 4;
pub const CAND_SCALARS: usize = 24;

/// What a candidate does (its kind token).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Ancient = 1,
    Event,
    Node,
    Claim,
    PickCard,
    Alternative1,
    Alternative2,
    Skip,
    Rest,
    UsePotion,
    Select,
    Confirm,
    Cancel,
    TakeRelic,
    BuyCard,
    BuyRelic,
    BuyPotion,
    Remove,
    Bundle,
    Proceed,
}

/// One legal run-level choice as the net sees it.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Its index in the run action space.
    pub index: u16,
    /// Its position in the legal list.
    pub position: usize,
    pub tokens: [u16; CAND_TOKENS],
    pub scalars: [f32; CAND_SCALARS],
}

/// A run decision's features.
#[derive(Clone, Debug)]
pub struct Features {
    pub tokens: [u16; STATE_TOKENS],
    pub scalars: [f32; STATE_SCALARS],
    pub candidates: Vec<Candidate>,
    /// The total floor (the reward's potential).
    pub floor: u8,
}

/// The run observation's bytes, read little-endian.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    const fn u8(&self, at: usize) -> u8 {
        self.0[at]
    }
    const fn u16(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.0[at], self.0[at + 1]])
    }
    const fn i16(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }
}

/// `offset + id`, `offset + 1` (unknown) past `size`; 0 stays 0 (an empty slot).
fn block(offset: usize, size: usize, id: usize) -> u16 {
    if id == 0 {
        return 0;
    }
    let id = if id < size { id } else { 1 };
    u16::try_from(offset + id).unwrap_or(0)
}

/// A card's token: its id and whether it is upgraded.
fn card_token(id: u16, upgrade: u8) -> u16 {
    if id == 0 {
        return 0;
    }
    let id = if usize::from(id) < CARD_SLOTS { id } else { 1 };
    u16::try_from(tok::CARD + 2 * usize::from(id) + usize::from(upgrade != 0)).unwrap_or(0)
}

/// What a vocabulary card id is: its type and whether it is a basic Strike or Defend.
#[derive(Clone, Copy, Debug, Default)]
struct CardInfo {
    kind: Option<CardType>,
    basic: bool,
}

fn card_info(id: u16) -> CardInfo {
    static TABLE: OnceLock<Vec<CardInfo>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = vec![CardInfo::default(); CARD_SLOTS];
        for kind in CardKind::ALL {
            let i = usize::from(vocab::card(kind));
            if let Some(info) = table.get_mut(i).filter(|_| i > 1) {
                *info = CardInfo {
                    kind: Some(kind.card_type()),
                    basic: kind.id().starts_with("STRIKE_") || kind.id().starts_with("DEFEND_"),
                };
            }
        }
        table
    });
    table.get(usize::from(id)).copied().unwrap_or_default()
}

/// Map rows the cone table holds.
const ROWS: usize = 64;
/// Map columns.
const COLS: usize = 8;
/// Room counts per path to the boss: elites, rests, shops, unknowns, monsters, treasures.
const CONE: usize = 6;
/// A node's forward cone: the min and max of each count over its paths to the boss, and its
/// rows to the boss.
#[derive(Clone, Copy, Debug)]
struct Cone {
    min: [u8; CONE],
    max: [u8; CONE],
    rows: u8,
}

/// The map section as a grid.
struct MapView {
    /// `(type + 1, children bitmask)` by row and column; 0 for no point.
    grid: [[(u8, u8); COLS]; ROWS],
    start: Option<(usize, usize)>,
    boss_row: usize,
    /// The legal next points' `(row, col)`.
    next: Vec<(usize, usize)>,
}

/// Map point type codes (`MapPointType` + 1).
mod room {
    pub const UNKNOWN: u8 = 2;
    pub const SHOP: u8 = 3;
    pub const TREASURE: u8 = 4;
    pub const REST: u8 = 5;
    pub const MONSTER: u8 = 6;
    pub const ELITE: u8 = 7;
    pub const BOSS: u8 = 8;
}

impl MapView {
    fn of(obs: &Bytes<'_>) -> Self {
        let mut view = Self {
            grid: [[(0, 0); COLS]; ROWS],
            start: None,
            boss_row: 0,
            next: Vec::new(),
        };
        for i in 0..MAP_POINTS {
            let at = MAP_AT + i * MAP_POINT_BYTES;
            let (col, row, kind) = (
                usize::from(obs.u8(at)),
                usize::from(obs.u8(at + 1)),
                obs.u8(at + 2),
            );
            if kind == 0 || row >= ROWS || col >= COLS {
                continue;
            }
            if i == 0 {
                view.start = Some((row, col));
            }
            if kind == room::BOSS && view.boss_row == 0 {
                view.boss_row = row;
            }
            view.grid[row][col] = (kind, obs.u8(at + 4));
            if obs.u8(at + 3) & 4 != 0 {
                view.next.push((row, col));
            }
        }
        view
    }

    /// The forward cones of every point, from the boss up.
    fn cones(&self) -> Vec<Option<Cone>> {
        let mut cones: Vec<Option<Cone>> = vec![None; ROWS * COLS];
        for row in (0..ROWS).rev() {
            for col in 0..COLS {
                let (kind, children) = self.grid[row][col];
                if kind == 0 {
                    continue;
                }
                if kind == room::BOSS {
                    cones[row * COLS + col] = Some(Cone {
                        min: [0; CONE],
                        max: [0; CONE],
                        rows: 0,
                    });
                    continue;
                }
                let mut acc: Option<Cone> = None;
                for c in 0..COLS {
                    if children & (1 << c) == 0 || row + 1 >= ROWS {
                        continue;
                    }
                    let Some(child) = cones[(row + 1) * COLS + c] else {
                        continue;
                    };
                    acc = Some(acc.map_or(child, |a| Cone {
                        min: std::array::from_fn(|k| a.min[k].min(child.min[k])),
                        max: std::array::from_fn(|k| a.max[k].max(child.max[k])),
                        rows: a.rows.max(child.rows),
                    }));
                }
                let Some(mut cone) = acc else { continue };
                let slot = match kind {
                    room::ELITE => Some(0),
                    room::REST => Some(1),
                    room::SHOP => Some(2),
                    room::UNKNOWN => Some(3),
                    room::MONSTER => Some(4),
                    room::TREASURE => Some(5),
                    _ => None,
                };
                if let Some(k) = slot {
                    cone.min[k] = cone.min[k].saturating_add(1);
                    cone.max[k] = cone.max[k].saturating_add(1);
                }
                cone.rows = cone.rows.saturating_add(1);
                cones[row * COLS + col] = Some(cone);
            }
        }
        cones
    }
}

/// Writes a cone into `out` (12 counts / 5, then rows / 17).
fn put_cone(cone: Option<Cone>, out: &mut [f32]) {
    let Some(cone) = cone else { return };
    for k in 0..CONE {
        out[2 * k] = f32::from(cone.min[k]) / 5.0;
        out[2 * k + 1] = f32::from(cone.max[k]) / 5.0;
    }
    out[2 * CONE] = f32::from(cone.rows) / 17.0;
}

impl Features {
    /// The features of a run in state `s` facing `decision` (not a fight) with the legal list
    /// `legal`. Only the run observation and the list are read.
    #[must_use]
    pub fn of(s: &RunState, decision: &RunDecision<'_>, legal: &[RunAction]) -> Self {
        let mut obs = vec![0; RUN_OBS_BYTES];
        RunObserver::new(false).observe_parts(s, decision, None, &mut obs);
        let mut indices: Vec<(usize, usize)> = Vec::new();
        for (position, action) in legal.iter().enumerate() {
            if let Some(index) = run_level_index(action)
                && !indices.iter().any(|&(i, _)| i == index)
            {
                indices.push((index, position));
            }
        }
        Self::from_obs(&obs, &indices)
    }

    /// The features of observation `obs` with the legal `(index, position)` pairs.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one flat read of the observation's sections"
    )]
    pub fn from_obs(obs: &[u8], indices: &[(usize, usize)]) -> Self {
        let obs = Bytes(obs);
        let mut tokens = [0_u16; STATE_TOKENS];
        let mut state = [0_f32; STATE_SCALARS];
        let hp = f32::from(obs.i16(8));
        let max_hp = f32::from(obs.i16(10)).max(1.0);
        let gold = f32::from(obs.i16(12));
        let act = usize::from(obs.u8(2)).min(2);
        let phase = obs.u8(3);
        state[0] = hp / max_hp;
        state[1] = hp / 100.0;
        state[2] = max_hp / 100.0;
        state[3] = gold / 300.0;
        state[4 + act] = 1.0;
        state[7] = f32::from(obs.u8(6)) / 17.0;
        state[8] = f32::from(obs.u8(7)) / 50.0;
        state[9] = f32::from(obs.u16(16)) / 30.0;
        state[10] = f32::from(obs.u8(18)) / 15.0;
        state[13] = f32::from(obs.u8(19)) / 3.0;
        state[14] = f32::from(obs.u8(14)) / 3.0;
        // The deck: tokens and counts.
        let mut deck_ids = Vec::with_capacity(DECK);
        let (mut upgraded, mut basics, mut curses, mut enchanted) = (0, 0, 0, 0);
        let mut types = [0_u32; 3];
        for i in 0..DECK {
            let at = DECK_AT + i * 4;
            let (id, upgrade, flags) = (obs.u16(at), obs.u8(at + 2), obs.u8(at + 3));
            if id == 0 {
                continue;
            }
            deck_ids.push(id);
            tokens[i] = card_token(id, upgrade);
            if flags & 1 != 0 {
                enchanted += 1;
                tokens[DECK + i] = block(tok::ENCHANT, 32, usize::from(flags >> 1).max(1));
            }
            upgraded += u32::from(upgrade != 0);
            let info = card_info(id);
            basics += u32::from(info.basic);
            match info.kind {
                Some(CardType::Curse) => curses += 1,
                Some(CardType::Attack) => types[0] += 1,
                Some(CardType::Skill) => types[1] += 1,
                Some(CardType::Power) => types[2] += 1,
                _ => {}
            }
        }
        state[15] = upgraded as f32 / 10.0;
        state[16] = basics as f32 / 10.0;
        state[17] = curses as f32 / 3.0;
        state[18] = types[0] as f32 / 10.0;
        state[19] = types[1] as f32 / 10.0;
        state[20] = types[2] as f32 / 5.0;
        state[21] = enchanted as f32 / 3.0;
        for i in 0..RELICS {
            tokens[DECK_TOKENS + i] =
                block(tok::RELIC, 512, usize::from(obs.u16(RELICS_AT + i * 4)));
        }
        let mut potions = 0;
        for i in 0..BELT.min(BELT_TOKENS) {
            let id = obs.u16(BELT_AT + i * 2);
            potions += u32::from(id != 0);
            tokens[DECK_TOKENS + RELIC_TOKENS + i] = block(tok::POTION, 128, usize::from(id));
        }
        state[11] = potions as f32 / 3.0;
        state[12] = (f32::from(obs.u8(15)) - potions as f32).max(0.0) / 3.0;
        let ctx = DECK_TOKENS + RELIC_TOKENS + BELT_TOKENS;
        tokens[ctx] = block(tok::ENCOUNTER, 128, usize::from(obs.u16(20)));
        tokens[ctx + 1] = block(tok::PHASE, 16, usize::from(phase) + 1);
        tokens[ctx + 2] = block(tok::ACT, 8, act + 1);
        tokens[ctx + 3] = match phase {
            0 => block(tok::ANCIENT, 32, usize::from(obs.u16(ANCIENT_AT))),
            1 => block(tok::EVENT, 128, usize::from(obs.u16(EVENT_AT))),
            5 => block(tok::PURPOSE, 32, usize::from(obs.u8(SELECT_AT))),
            _ => 0,
        };
        // The map ahead.
        let map = MapView::of(&obs);
        let cones = map.cones();
        let (col, row) = (usize::from(obs.u8(24)), usize::from(obs.u8(25)));
        let here = if row < ROWS && col < COLS && map.grid[row][col].0 != 0 {
            Some((row, col))
        } else {
            map.start
        };
        let cone_of = |(r, c): (usize, usize)| cones.get(r * COLS + c).copied().flatten();
        put_cone(here.and_then(cone_of), &mut state[22..35]);
        state[35] = f32::from(u8::from(
            here.is_some_and(|(r, _)| r + 1 == map.boss_row && map.boss_row > 0),
        ));
        state[36] = f32::from(u8::from(phase == 9));
        state[37] = f32::from(obs.u8(0)) / 4.0;
        state[38] = deck_ids.len() as f32 / 30.0;
        state[39] = 1.0;

        let copies = |id: u16| deck_ids.iter().filter(|&&d| d == id).count() as f32 / 3.0;
        let card_scalars = |id: u16, upgrade: u8, cscal: &mut [f32; CAND_SCALARS]| {
            cscal[3] = f32::from(u8::from(upgrade != 0));
            cscal[4] = copies(id);
            let info = card_info(id);
            cscal[22] = f32::from(u8::from(info.basic));
            cscal[23] = f32::from(u8::from(matches!(
                info.kind,
                Some(CardType::Curse | CardType::Status)
            )));
        };
        let price_scalars = |price: i16, cscal: &mut [f32; CAND_SCALARS]| {
            let price = f32::from(price);
            cscal[0] = price / 300.0;
            cscal[1] = f32::from(u8::from(price <= gold));
            cscal[2] = (gold - price) / 300.0;
        };
        let phase_token = block(tok::PHASE, 16, usize::from(phase) + 1);
        let mut candidates = Vec::with_capacity(indices.len());
        for &(index, position) in indices {
            let mut ctok = [0_u16; CAND_TOKENS];
            let mut cscal = [0_f32; CAND_SCALARS];
            let kind = if (ANCIENT_BASE..EVENT_BASE).contains(&index) {
                let at = ANCIENT_AT + 2 + (index - ANCIENT_BASE) * 3;
                ctok[1] = block(tok::RELIC, 512, usize::from(obs.u16(at)));
                ctok[2] = block(tok::ANCIENT, 32, usize::from(obs.u16(ANCIENT_AT)));
                cscal[5] = f32::from(obs.u8(at + 2));
                Kind::Ancient
            } else if (EVENT_BASE..NODE_BASE).contains(&index) {
                let at = EVENT_AT + 2 + (index - EVENT_BASE) * 3;
                let text = usize::from(obs.u16(at));
                if text != 0 {
                    ctok[1] = u16::try_from(tok::TEXT + text % tok::TEXT_BUCKETS).unwrap_or(0);
                }
                ctok[2] = block(tok::EVENT, 128, usize::from(obs.u16(EVENT_AT)));
                cscal[5] = f32::from(obs.u8(at + 2));
                Kind::Event
            } else if (NODE_BASE..CLAIM_BASE).contains(&index) {
                let col = index - NODE_BASE;
                if let Some(&(r, cl)) = map.next.iter().find(|&&(_, cl)| cl == col) {
                    ctok[1] = block(tok::ROOM, 16, usize::from(map.grid[r][cl].0));
                    put_cone(cone_of((r, cl)), &mut cscal[8..21]);
                }
                Kind::Node
            } else if (CLAIM_BASE..PICK_CARD_BASE).contains(&index) {
                let at = REWARDS_AT + (index - CLAIM_BASE) * REWARD_BYTES;
                let (kind, value) = (obs.u8(at), obs.u16(at + 2));
                ctok[1] = block(tok::REWARD, 8, usize::from(kind));
                ctok[2] = match kind {
                    2 => block(tok::POTION, 128, usize::from(value)),
                    3 | 4 => block(tok::RELIC, 512, usize::from(value)),
                    6 => card_token(value, obs.u8(at + 4)),
                    _ => 0,
                };
                if kind == 1 {
                    cscal[6] = f32::from(value) / 100.0;
                }
                if kind == 6 {
                    card_scalars(value, obs.u8(at + 4), &mut cscal);
                }
                Kind::Claim
            } else if (PICK_CARD_BASE..ALTERNATIVE_BASE).contains(&index) {
                let i = index - PICK_CARD_BASE;
                let (r, k) = (i / REWARD_CARD_SLOTS, i % REWARD_CARD_SLOTS);
                let at = REWARDS_AT + r * REWARD_BYTES + 6 + k * 3;
                let (id, upgrade) = (obs.u16(at), obs.u8(at + 2));
                ctok[1] = card_token(id, upgrade);
                card_scalars(id, upgrade, &mut cscal);
                Kind::PickCard
            } else if (ALTERNATIVE_BASE..SKIP_BASE).contains(&index) {
                let i = index - ALTERNATIVE_BASE;
                let at = REWARDS_AT + (i / ALTERNATIVE_SLOTS) * REWARD_BYTES;
                ctok[1] = block(tok::REWARD, 8, usize::from(obs.u8(at)));
                if i.is_multiple_of(ALTERNATIVE_SLOTS) {
                    Kind::Alternative1
                } else {
                    Kind::Alternative2
                }
            } else if (SKIP_BASE..REST_BASE).contains(&index) {
                let at = REWARDS_AT + (index - SKIP_BASE) * REWARD_BYTES;
                ctok[1] = block(tok::REWARD, 8, usize::from(obs.u8(at)));
                Kind::Skip
            } else if (REST_BASE..USE_POTION_BASE).contains(&index) {
                let at = REST_AT + (index - REST_BASE) * 2;
                ctok[1] = block(tok::REST, 16, usize::from(obs.u8(at)));
                cscal[5] = f32::from(obs.u8(at + 1));
                Kind::Rest
            } else if (USE_POTION_BASE..SELECT_BASE).contains(&index) {
                let slot = index - USE_POTION_BASE;
                if slot < BELT {
                    ctok[1] = block(tok::POTION, 128, usize::from(obs.u16(BELT_AT + slot * 2)));
                }
                Kind::UsePotion
            } else if (SELECT_BASE..CONFIRM_SELECT).contains(&index) {
                let at = SELECT_AT + 8 + (index - SELECT_BASE) * SELECT_OPTION_BYTES;
                let (id, upgrade, deck_index, flags) =
                    (obs.u16(at), obs.u8(at + 2), obs.u8(at + 3), obs.u8(at + 4));
                ctok[1] = card_token(id, upgrade);
                ctok[2] = block(tok::PURPOSE, 32, usize::from(obs.u8(SELECT_AT)));
                if usize::from(deck_index) < DECK {
                    ctok[3] = tokens[DECK + usize::from(deck_index)];
                }
                card_scalars(id, upgrade, &mut cscal);
                cscal[7] = f32::from(flags & 1);
                cscal[21] = f32::from(obs.u8(SELECT_AT + 2)) / 5.0;
                Kind::Select
            } else if index == CONFIRM_SELECT || index == CANCEL_SELECT || index == PROCEED {
                ctok[1] = phase_token;
                ctok[2] = tokens[ctx + 3];
                cscal[7] = f32::from(obs.u8(SELECT_AT + 3)) / 5.0;
                match index {
                    CONFIRM_SELECT => Kind::Confirm,
                    CANCEL_SELECT => Kind::Cancel,
                    _ => Kind::Proceed,
                }
            } else if index == TAKE_RELIC {
                ctok[1] = block(tok::RELIC, 512, usize::from(obs.u16(TREASURE_AT)));
                Kind::TakeRelic
            } else if (BUY_CARD_BASE..BUY_RELIC_BASE).contains(&index) {
                let at = SHOP_AT + (index - BUY_CARD_BASE) * 6;
                let (id, upgrade) = (obs.u16(at), obs.u8(at + 2));
                ctok[1] = card_token(id, upgrade);
                card_scalars(id, upgrade, &mut cscal);
                price_scalars(obs.i16(at + 4), &mut cscal);
                cscal[5] = f32::from(obs.u8(at + 3));
                Kind::BuyCard
            } else if (BUY_RELIC_BASE..BUY_POTION_BASE).contains(&index) {
                let at = SHOP_AT + SHOP_CARDS * 6 + (index - BUY_RELIC_BASE) * 4;
                ctok[1] = block(tok::RELIC, 512, usize::from(obs.u16(at)));
                price_scalars(obs.i16(at + 2), &mut cscal);
                Kind::BuyRelic
            } else if (BUY_POTION_BASE..REMOVE_CARD).contains(&index) {
                let at = SHOP_AT + SHOP_CARDS * 6 + SHOP_RELICS * 4 + (index - BUY_POTION_BASE) * 4;
                ctok[1] = block(tok::POTION, 128, usize::from(obs.u16(at)));
                price_scalars(obs.i16(at + 2), &mut cscal);
                Kind::BuyPotion
            } else if index == REMOVE_CARD {
                let at = SHOP_AT + SHOP_CARDS * 6 + (SHOP_RELICS + SHOP_POTIONS) * 4;
                price_scalars(obs.i16(at), &mut cscal);
                Kind::Remove
            } else if (BUNDLE_BASE..PROCEED).contains(&index) {
                let at = BUNDLES_AT + (index - BUNDLE_BASE) * BUNDLE_CARDS * 3;
                for k in 0..(CAND_TOKENS - 1).min(BUNDLE_CARDS) {
                    ctok[1 + k] = card_token(obs.u16(at + k * 3), obs.u8(at + k * 3 + 2));
                }
                Kind::Bundle
            } else {
                continue;
            };
            ctok[0] = block(tok::KIND, 32, kind as usize);
            candidates.push(Candidate {
                index: u16::try_from(index).unwrap_or(u16::MAX),
                position,
                tokens: ctok,
                scalars: cscal,
            });
        }
        // A deck selection with a card left to pick offers neither Cancel nor un-picking: the
        // greedy policy looped on them (Smith, Cancel, Smith; RLB8), and the hand rules never
        // take them either.
        let select = |c: &Candidate| (SELECT_BASE..CONFIRM_SELECT).contains(&usize::from(c.index));
        if candidates.iter().any(|c| select(c) && c.scalars[7] == 0.0) {
            candidates.retain(|c| {
                usize::from(c.index) != CANCEL_SELECT && !(select(c) && c.scalars[7] != 0.0)
            });
        }
        Self {
            tokens,
            scalars: state,
            candidates,
            floor: obs.u8(7),
        }
    }
}

/// The export's magic bytes.
const MAGIC: &[u8; 8] = b"STS2RUN1";
/// The committed export.
const BUILTIN: &[u8] = include_bytes!("../../data/run-policy.bin");
/// Embedding width, hidden width, and the scorer's second width.
const HEADER_WORDS: usize = 9;

/// A dense layer, `out × in` row-major.
#[derive(Clone, Debug)]
struct Layer {
    inputs: usize,
    weights: Vec<f32>,
    bias: Vec<f32>,
}

impl Layer {
    fn forward_into(&self, x: &[f32], out: &mut Vec<f32>, relu: bool) {
        out.clear();
        out.extend(
            self.bias
                .iter()
                .zip(self.weights.chunks_exact(self.inputs))
                .map(|(&b, row)| {
                    let y = b + row.iter().zip(x).map(|(w, v)| w * v).sum::<f32>();
                    if relu { y.max(0.0) } else { y }
                }),
        );
    }

    fn forward(&self, x: &[f32], relu: bool) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.bias.len());
        self.forward_into(x, &mut out, relu);
        out
    }
}

/// The run policy net (`python/train_run_ppo.py`, `RunNet`).
#[derive(Clone, Debug)]
pub struct RunNet {
    embed: usize,
    table: Vec<f32>,
    l1: Layer,
    l2: Layer,
    v1: Layer,
    v2: Layer,
    lh: Layer,
    /// The candidate projection (no bias: `lh` has it).
    lc: Layer,
    l3: Layer,
    l4: Layer,
}

/// Pool scales of the state's token bags: deck, relics, belt, context.
const POOL_SCALE: [f32; 4] = [0.1, 0.2, 1.0, 1.0];

impl RunNet {
    /// Reads the export at `path`.
    ///
    /// # Errors
    /// If the file is unreadable, malformed or for another feature layout.
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&bytes).map_err(|e| format!("{path}: {e}"))
    }

    /// Parses an export.
    ///
    /// # Errors
    /// If it is malformed or for another feature layout.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let (magic, rest) = bytes.split_at_checked(8).ok_or("too short")?;
        if magic != MAGIC {
            return Err("not a run policy export".to_owned());
        }
        let (header, rest) = rest
            .split_at_checked(HEADER_WORDS * 4 + 8)
            .ok_or("too short")?;
        let word = |i: usize| {
            usize::try_from(u32::from_le_bytes([
                header[4 * i],
                header[4 * i + 1],
                header[4 * i + 2],
                header[4 * i + 3],
            ]))
            .unwrap_or(usize::MAX)
        };
        let shape = [
            FEATURE_VERSION as usize,
            TOKENS,
            STATE_TOKENS,
            STATE_SCALARS,
            CAND_TOKENS,
            CAND_SCALARS,
        ];
        let got: Vec<usize> = (0..6).map(word).collect();
        if got != shape {
            return Err(format!("layout {got:?}, this build has {shape:?}"));
        }
        // The vocabulary hash after the words is informational: the token blocks have fixed
        // sizes and vocabulary ids are append-only, so an export stays valid as it grows.
        let (embed, hidden, second) = (word(6), word(7), word(8));
        let floats: Vec<f32> = rest
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        Self::from_floats(embed, hidden, second, &floats)
    }

    fn from_floats(
        embed: usize,
        hidden: usize,
        second: usize,
        floats: &[f32],
    ) -> Result<Self, String> {
        let mut at = 0;
        let mut take = |n: usize| -> Result<Vec<f32>, String> {
            let v = floats.get(at..at + n).ok_or("too short")?.to_vec();
            at += n;
            Ok(v)
        };
        let table = take(TOKENS * embed)?;
        let mut layer = |inputs: usize, outputs: usize, bias: bool| -> Result<Layer, String> {
            let weights = take(inputs * outputs)?;
            let bias = if bias {
                take(outputs)?
            } else {
                vec![0.0; outputs]
            };
            Ok(Layer {
                inputs,
                weights,
                bias,
            })
        };
        let state_in = STATE_SCALARS + 4 * embed;
        let net = Self {
            embed,
            l1: layer(state_in, hidden, true)?,
            l2: layer(hidden, hidden, true)?,
            v1: layer(hidden, second, true)?,
            v2: layer(second, 1, true)?,
            lh: layer(hidden, hidden, true)?,
            lc: layer(embed + CAND_SCALARS, hidden, false)?,
            l3: layer(hidden, second, true)?,
            l4: layer(second, 1, true)?,
            table,
        };
        if at != floats.len() {
            return Err(format!("{} trailing floats", floats.len() - at));
        }
        Ok(net)
    }

    /// The committed weights (`crates/sts2-diff/data/run-policy.md`).
    ///
    /// # Panics
    /// If they are for another feature layout (retrain and re-export).
    #[must_use]
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("the committed run policy matches this build")
    }

    /// A net with small pseudo-random weights from `seed` (for tests).
    ///
    /// # Panics
    /// Never: the sizes are this build's.
    #[must_use]
    pub fn random(seed: u64) -> Self {
        let (embed, hidden, second) = (8, 16, 8);
        let n = TOKENS * embed
            + (STATE_SCALARS + 4 * embed) * hidden
            + hidden
            + hidden * hidden
            + hidden
            + hidden * second
            + second
            + second
            + 1
            + hidden * hidden
            + hidden
            + (embed + CAND_SCALARS) * hidden
            + hidden * second
            + second
            + second
            + 1;
        let mut rng = SplitMix(seed);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "small test weights; any f32 near the value will do"
        )]
        let floats: Vec<f32> = (0..n).map(|_| (rng.unit() as f32 - 0.5) * 0.5).collect();
        Self::from_floats(embed, hidden, second, &floats).expect("the sizes add up")
    }

    fn pool(&self, tokens: &[u16], scale: f32, out: &mut Vec<f32>) {
        let start = out.len();
        out.resize(start + self.embed, 0.0);
        for &t in tokens.iter().filter(|&&t| t != 0) {
            let row = &self.table[usize::from(t) * self.embed..][..self.embed];
            for (o, w) in out[start..].iter_mut().zip(row) {
                *o += w;
            }
        }
        for o in &mut out[start..] {
            *o *= scale;
        }
    }

    /// The candidates' logits and the state's value.
    #[must_use]
    pub fn evaluate(&self, features: &Features) -> (Vec<f32>, f32) {
        let mut input = features.scalars.to_vec();
        let bounds = [
            0,
            DECK_TOKENS,
            DECK_TOKENS + RELIC_TOKENS,
            DECK_TOKENS + RELIC_TOKENS + BELT_TOKENS,
            STATE_TOKENS,
        ];
        for (k, scale) in POOL_SCALE.iter().enumerate() {
            self.pool(
                &features.tokens[bounds[k]..bounds[k + 1]],
                *scale,
                &mut input,
            );
        }
        let hidden = self.l2.forward(&self.l1.forward(&input, true), true);
        let value = self.v2.forward(&self.v1.forward(&hidden, true), false)[0];
        let hp = self.lh.forward(&hidden, false);
        let mut cand = Vec::with_capacity(self.embed + CAND_SCALARS);
        let mut z1 = Vec::new();
        let mut z2 = Vec::new();
        let logits = features
            .candidates
            .iter()
            .map(|c| {
                cand.clear();
                self.pool(&c.tokens, 1.0, &mut cand);
                cand.extend_from_slice(&c.scalars);
                self.lc.forward_into(&cand, &mut z1, false);
                for (zi, hi) in z1.iter_mut().zip(&hp) {
                    *zi = (*zi + hi).max(0.0);
                }
                self.l3.forward_into(&z1, &mut z2, true);
                self.l4.forward(&z2, false)[0]
            })
            .collect();
        (logits, value)
    }

    /// The greedy action among `legal` for a run in `s` facing `decision` (not a fight); `None`
    /// when no legal action has a run-level index.
    #[must_use]
    pub fn choose(
        &self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
    ) -> Option<RunAction> {
        let f = Features::of(s, decision, legal);
        let (logits, _) = self.evaluate(&f);
        let best = argmax(&logits)?;
        legal.get(f.candidates[best].position).cloned()
    }

    /// [`RunNet::choose`] with the policy's probability of its choice (the softmax over the
    /// candidates).
    #[must_use]
    pub fn choose_sure(
        &self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
    ) -> Option<(RunAction, f32)> {
        let f = Features::of(s, decision, legal);
        let (logits, _) = self.evaluate(&f);
        let best = argmax(&logits)?;
        let top = logits[best];
        let total: f32 = logits.iter().map(|l| (l - top).exp()).sum();
        let action = legal.get(f.candidates[best].position).cloned()?;
        Some((action, 1.0 / total))
    }

    /// The policy's probability of each candidate (the softmax [`RunNet::choose_sure`] reads),
    /// with its action's run-level index and position in `legal`.
    #[must_use]
    pub fn shares(
        &self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
    ) -> Vec<(usize, usize, f32)> {
        let f = Features::of(s, decision, legal);
        let (logits, _) = self.evaluate(&f);
        let top = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let total: f32 = logits.iter().map(|l| (l - top).exp()).sum();
        f.candidates
            .iter()
            .zip(&logits)
            .map(|(c, l)| (usize::from(c.index), c.position, (l - top).exp() / total))
            .collect()
    }
}

/// The first index of the largest value.
fn argmax(xs: &[f32]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, &x) in xs.iter().enumerate() {
        if best.is_none_or(|(_, b)| x > b) {
            best = Some((i, x));
        }
    }
    best.map(|(i, _)| i)
}

/// A sample from the softmax of `logits` (non-empty) with `u` uniform in [0, 1): the index
/// and its log-probability.
#[must_use]
pub fn sample(logits: &[f32], u: f64) -> (usize, f32) {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f64> = logits.iter().map(|&l| f64::from(l - max).exp()).collect();
    let total: f64 = exps.iter().sum();
    let mut acc = 0.0;
    let mut chosen = exps.len() - 1;
    for (i, e) in exps.iter().enumerate() {
        acc += e / total;
        if u < acc {
            chosen = i;
            break;
        }
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a log-probability; f32 is what the trainer reads"
    )]
    let logp = (exps[chosen] / total).ln() as f32;
    (chosen, logp)
}

/// A small deterministic generator for the sampled actions.
#[derive(Clone, Debug)]
pub struct SplitMix(pub u64);

impl SplitMix {
    pub const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// One recorded decision.
#[derive(Clone, Debug)]
pub struct Record {
    pub features: Features,
    /// The candidate taken.
    pub chosen: usize,
    /// Its log-probability under the collecting policy (0 for the hand rules).
    pub logp: f32,
}

/// Who makes the run decisions while collecting.
#[derive(Clone, Copy, Debug)]
pub enum Collector<'a> {
    /// The hand rules ([`super::run`]), recorded for behaviour cloning.
    Hand,
    /// The net, sampled.
    Sample(&'a RunNet),
    /// The net, greedy.
    Greedy(&'a RunNet),
}

impl Collector<'_> {
    /// The action for a run decision (not a fight) among `legal` (the actions the engine
    /// accepts), and its record when the decision had two or more candidates.
    pub fn choose(
        self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
        rng: &mut SplitMix,
    ) -> (RunAction, Option<Record>) {
        let f = Features::of(s, decision, legal);
        let hand = || super::run::choose(s, decision, legal);
        if f.candidates.len() < 2 {
            return (hand(), None);
        }
        let (chosen, logp) = match self {
            Collector::Hand => {
                let action = hand();
                let index = run_level_index(&action);
                let Some(chosen) = f
                    .candidates
                    .iter()
                    .position(|c| Some(usize::from(c.index)) == index)
                else {
                    return (action, None);
                };
                (chosen, 0.0)
            }
            Collector::Sample(net) => sample(&net.evaluate(&f).0, rng.unit()),
            Collector::Greedy(net) => (argmax(&net.evaluate(&f).0).unwrap_or(0), 0.0),
        };
        let action = legal[f.candidates[chosen].position].clone();
        (
            action,
            Some(Record {
                features: f,
                chosen,
                logp,
            }),
        )
    }
}

/// Recorded decisions as the trainer reads them: little-endian arrays, one file each.
#[derive(Debug, Default)]
pub struct RecordFiles {
    /// Per decision: run, first candidate, candidates, chosen, floor (`u32`), log-probability
    /// (`f32` bits).
    dec: Vec<u8>,
    dec_tok: Vec<u8>,
    dec_scal: Vec<u8>,
    cand_tok: Vec<u8>,
    cand_scal: Vec<u8>,
    cand_idx: Vec<u8>,
    candidates: u32,
}

impl RecordFiles {
    /// Appends run `run`'s records.
    pub fn push(&mut self, run: u32, records: &[Record]) {
        for r in records {
            let f = &r.features;
            let n = u32::try_from(f.candidates.len()).unwrap_or(u32::MAX);
            for word in [
                run,
                self.candidates,
                n,
                u32::try_from(r.chosen).unwrap_or(u32::MAX),
                u32::from(f.floor),
                r.logp.to_bits(),
            ] {
                self.dec.extend_from_slice(&word.to_le_bytes());
            }
            self.candidates += n;
            self.dec_tok
                .extend(f.tokens.iter().flat_map(|t| t.to_le_bytes()));
            self.dec_scal
                .extend(f.scalars.iter().flat_map(|x| x.to_le_bytes()));
            for c in &f.candidates {
                self.cand_tok
                    .extend(c.tokens.iter().flat_map(|t| t.to_le_bytes()));
                self.cand_scal
                    .extend(c.scalars.iter().flat_map(|x| x.to_le_bytes()));
                self.cand_idx.extend_from_slice(&c.index.to_le_bytes());
            }
        }
    }

    /// Writes `dec.bin`, `dec_tok.bin`, `dec_scal.bin`, `cand_tok.bin`, `cand_scal.bin`,
    /// `cand_idx.bin` and the layout, `meta.json`, under `dir`.
    ///
    /// # Errors
    /// If a file cannot be written.
    pub fn write(&self, dir: &str) -> std::io::Result<()> {
        for (name, bytes) in [
            ("dec", &self.dec),
            ("dec_tok", &self.dec_tok),
            ("dec_scal", &self.dec_scal),
            ("cand_tok", &self.cand_tok),
            ("cand_scal", &self.cand_scal),
            ("cand_idx", &self.cand_idx),
        ] {
            std::fs::write(format!("{dir}/{name}.bin"), bytes)?;
        }
        let meta = serde_json::json!({
            "feature_version": FEATURE_VERSION,
            "tokens": TOKENS,
            "state_tokens": STATE_TOKENS,
            "deck_tokens": DECK_TOKENS,
            "relic_tokens": RELIC_TOKENS,
            "belt_tokens": BELT_TOKENS,
            "context_tokens": CONTEXT_TOKENS,
            "state_scalars": STATE_SCALARS,
            "cand_tokens": CAND_TOKENS,
            "cand_scalars": CAND_SCALARS,
            "vocab_hash": format!("{:016x}", vocab::hash()),
        });
        std::fs::write(format!("{dir}/meta.json"), meta.to_string())
    }
}
