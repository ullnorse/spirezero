//! WEB83: lethal this turn, and how the net uses it (a measurement the parity tests report).
//!
//! [`lethal`] searches the turn's card plays and choices (no potions, no End Turn) on copies of
//! the run, at most [`NODES`] positions, for a line that wins the fight. A line counts as honest
//! when it also wins on two determinized copies (draw pile reshuffled, fight streams reseeded,
//! [`Run::determinize_combat`]): it then needs only the hand, the energy and what the player
//! sees. [`Study`] follows each fight's turns and counts the turns with a lethal line the net
//! did not take.

use std::collections::HashSet;
use std::fmt::Write as _;

use sts2_core::combat::{Action, CardId, CreatureId};
use sts2_core::content::cards::{PreviewMode, TargetType, Var, VarClass};
use sts2_core::run::{Run, RunAction, RunDecision};

/// Positions searched per turn.
pub const NODES: usize = 4000;

/// A lethal line.
#[derive(Clone, Debug)]
pub struct Lethal {
    /// Cards played.
    pub cards: usize,
    /// Whether it plays an Osty attack.
    pub osty: bool,
    /// The line's previewed damage minus the enemies' HP and block at the turn's start.
    pub margin: i32,
    pub honest: bool,
}

fn won(run: &Run) -> bool {
    !matches!(
        run.pending_decision(),
        RunDecision::Combat(_) | RunDecision::GameOver
    )
}

/// The previewed damage of playing `card` at `target` (every enemy for an `AllEnemies` card).
fn damage(run: &Run, card: CardId, target: Option<CreatureId>) -> i32 {
    let s = run.combat().expect("a fight").state();
    let kind = s.card(card).kind;
    let values = s.card_preview(card, target, PreviewMode::Normal);
    let hit = kind
        .vars()
        .iter()
        .zip(&values)
        .find(|(v, _)| {
            matches!(
                v.class,
                VarClass::Damage(_) | VarClass::OstyDamage(_) | VarClass::CalculatedDamage { .. }
            )
        })
        .map_or(0, |(_, &(_, v))| v.to_int_truncating());
    let hits = values
        .iter()
        .find(|&&(v, _)| v == Var::Repeat)
        .map_or(1, |&(_, v)| v.to_int_truncating());
    let targets = if kind.target() == TargetType::AllEnemies {
        i32::try_from(
            s.enemies
                .iter()
                .filter(|&&e| s.creature(e).is_alive())
                .count(),
        )
        .unwrap_or(1)
    } else {
        1
    };
    hit * hits * targets
}

/// The enemies' HP and block.
fn enemy_pool(run: &Run) -> i32 {
    let s = run.combat().expect("a fight").state();
    s.enemies
        .iter()
        .map(|&e| s.creature(e))
        .filter(|c| c.is_alive())
        .map(|c| c.hp + c.block)
        .sum()
}

struct Search<'a> {
    root: &'a Run,
    pool: i32,
    nodes: usize,
    best: Option<Lethal>,
    line: Vec<RunAction>,
}

impl Search<'_> {
    fn replays(&self, seed: u64) -> bool {
        let mut run = self.root.clone();
        run.determinize_combat(seed);
        self.line.iter().all(|a| run.apply(a.clone()).is_ok()) && won(&run)
    }

    fn dfs(&mut self, run: &Run, dealt: i32, cards: usize, osty: bool) {
        if self.nodes >= NODES
            || self
                .best
                .as_ref()
                .is_some_and(|b| b.honest && b.margin >= 10)
        {
            return;
        }
        self.nodes += 1;
        let mut seen = HashSet::new();
        for a in run.legal_actions() {
            let (key, add, card_osty) = match &a {
                RunAction::Combat(Action::PlayCard { card, target }) => {
                    let s = run.combat().expect("a fight").state();
                    let c = s.card(*card);
                    let mut key = String::new();
                    let _ = write!(key, "{:?}{}{target:?}", c.kind, c.upgrade_level);
                    (key, damage(run, *card, *target), c.kind.is_osty_attack())
                }
                RunAction::Combat(Action::Resolve(_)) => (format!("{a:?}"), 0, false),
                _ => continue,
            };
            if !seen.insert(key) {
                continue;
            }
            let mut next = run.clone();
            if next.apply(a.clone()).is_err() {
                continue;
            }
            let played = usize::from(matches!(a, RunAction::Combat(Action::PlayCard { .. })));
            self.line.push(a);
            if won(&next) {
                let found = Lethal {
                    cards: cards + played,
                    osty: osty || card_osty,
                    margin: dealt + add - self.pool,
                    honest: self.replays(1) && self.replays(2),
                };
                if self
                    .best
                    .as_ref()
                    .is_none_or(|b| (found.honest, found.margin) > (b.honest, b.margin))
                {
                    self.best = Some(found);
                }
            } else if matches!(next.pending_decision(), RunDecision::Combat(_)) {
                self.dfs(&next, dealt + add, cards + played, osty || card_osty);
            }
            self.line.pop();
        }
    }
}

/// The best lethal line this turn (honest first, then the widest margin), and whether the
/// search hit its cap.
pub fn lethal(run: &Run) -> (Option<Lethal>, bool) {
    let mut s = Search {
        root: run,
        pool: enemy_pool(run),
        nodes: 0,
        best: None,
        line: Vec::new(),
    };
    s.dfs(run, 0, 0, false);
    (s.best, s.nodes >= NODES)
}

/// A fight's lethal turn not taken (yet).
struct Candidate {
    line: Lethal,
    row: Vec<u8>,
}

#[derive(Default)]
struct Track {
    open: bool,
    turn: Option<u32>,
    first: Option<u32>,
    cand: Option<Candidate>,
    honest_misses: usize,
}

/// Counts over the fights a test plays.
#[derive(Default, Debug)]
pub struct Counts {
    pub turns: usize,
    pub capped: usize,
    pub lethal_turns: usize,
    pub honest_lethal_turns: usize,
    pub taken: usize,
    pub missed: usize,
    pub honest_missed: usize,
    /// Honest misses by margin 0-2, 3-9, 10+.
    pub missed_margin: [usize; 3],
    /// Honest lethal turns taken, by the same margins.
    pub taken_margin: [usize; 3],
    /// Honest misses by cards needed 1, 2, 3, 4+.
    pub missed_cards: [usize; 4],
    pub taken_cards: [usize; 4],
    pub missed_osty: usize,
    pub taken_osty: usize,
    pub fights: usize,
    pub died: usize,
    /// Deaths after an honest missed lethal in the same fight (what a lethal check fixes).
    pub died_after_miss: usize,
    pub fights_with_lethal: usize,
    pub fights_with_miss: usize,
    /// Won fights with a lethal turn: turns from the first lethal turn to the kill.
    pub delay: Vec<u32>,
}

const fn margin_bucket(m: i32) -> usize {
    match m {
        ..=2 => 0,
        3..=9 => 1,
        _ => 2,
    }
}

/// One missed turn for the value check: its env row, the next turn's, the line's margin.
pub type MissRow = (Vec<u8>, Vec<u8>, i32);

/// The study over a batch of envs; `rows` keeps up to `keep_rows` missed turns.
pub struct Study {
    tracks: Vec<Track>,
    pub counts: Counts,
    pub rows: Vec<MissRow>,
    keep_rows: usize,
}

impl Study {
    pub fn new(envs: usize, keep_rows: usize) -> Self {
        Self {
            tracks: (0..envs).map(|_| Track::default()).collect(),
            counts: Counts::default(),
            rows: Vec::new(),
            keep_rows,
        }
    }

    /// A fight decision of env `i` at `run`; `row` is its env row (obs, mask, priv bytes).
    pub fn decision(&mut self, i: usize, run: &Run, row: &[u8]) {
        let turn = run.combat().expect("a fight").state().player.turn_number;
        let t = &mut self.tracks[i];
        t.open = true;
        if t.turn == Some(turn) {
            return;
        }
        if let Some(c) = t.cand.take() {
            Self::miss(
                &mut self.counts,
                &mut self.rows,
                self.keep_rows,
                t,
                c,
                Some(row),
            );
        }
        t.turn = Some(turn);
        self.counts.turns += 1;
        let (line, capped) = lethal(run);
        self.counts.capped += usize::from(capped);
        if let Some(line) = line {
            self.counts.lethal_turns += 1;
            if line.honest {
                self.counts.honest_lethal_turns += 1;
                t.first.get_or_insert(turn);
            }
            t.cand = Some(Candidate {
                line,
                row: row.to_vec(),
            });
        }
    }

    fn miss(
        counts: &mut Counts,
        rows: &mut Vec<MissRow>,
        keep: usize,
        t: &mut Track,
        c: Candidate,
        next: Option<&[u8]>,
    ) {
        counts.missed += 1;
        if !c.line.honest {
            return;
        }
        counts.honest_missed += 1;
        t.honest_misses += 1;
        counts.missed_margin[margin_bucket(c.line.margin)] += 1;
        counts.missed_cards[c.line.cards.clamp(1, 4) - 1] += 1;
        counts.missed_osty += usize::from(c.line.osty);
        if rows.len() < keep
            && let Some(next) = next
        {
            rows.push((c.row, next.to_vec(), c.line.margin));
        }
    }

    /// Env `i`'s fight ended: `died`, or won.
    pub fn end(&mut self, i: usize, died: bool) {
        let t = &mut self.tracks[i];
        if !t.open {
            return;
        }
        if let Some(c) = t.cand.take() {
            if died {
                Self::miss(&mut self.counts, &mut self.rows, self.keep_rows, t, c, None);
            } else {
                self.counts.taken += 1;
                if c.line.honest {
                    self.counts.taken_margin[margin_bucket(c.line.margin)] += 1;
                    self.counts.taken_cards[c.line.cards.clamp(1, 4) - 1] += 1;
                    self.counts.taken_osty += usize::from(c.line.osty);
                }
            }
        }
        let c = &mut self.counts;
        c.fights += 1;
        c.died += usize::from(died);
        c.died_after_miss += usize::from(died && t.honest_misses > 0);
        c.fights_with_lethal += usize::from(t.first.is_some());
        c.fights_with_miss += usize::from(t.honest_misses > 0);
        if let (false, Some(first), Some(kill)) = (died, t.first, t.turn) {
            c.delay.push(kill - first);
        }
        *t = Track::default();
    }

    /// The report.
    pub fn report(&self) -> String {
        let c = &self.counts;
        let mut d = c.delay.clone();
        d.sort_unstable();
        let mean = if d.is_empty() {
            0.0
        } else {
            f64::from(d.iter().sum::<u32>()) / d.len() as f64
        };
        let p = |q: f64| {
            d.get(((d.len() as f64 - 1.0) * q).round() as usize)
                .copied()
                .unwrap_or(0)
        };
        let mut hist = [0usize; 6];
        for &x in &d {
            hist[(x as usize).min(5)] += 1;
        }
        format!(
            "turns {} (search capped {}), lethal turns {} (honest {}), taken {}, missed {} (honest {})\n  \
             honest missed by margin [0-2, 3-9, 10+] {:?} vs taken {:?}\n  \
             honest missed by cards [1, 2, 3, 4+] {:?} vs taken {:?}; with an Osty attack missed {} taken {}\n  \
             fights {} died {} died after an honest missed lethal {}; fights with honest lethal {} with a miss {}\n  \
             delay first honest lethal -> kill (won fights): n={} mean {mean:.2} median {} p90 {} max {} hist[0..5+] {hist:?}",
            c.turns,
            c.capped,
            c.lethal_turns,
            c.honest_lethal_turns,
            c.taken,
            c.missed,
            c.honest_missed,
            c.missed_margin,
            c.taken_margin,
            c.missed_cards,
            c.taken_cards,
            c.missed_osty,
            c.taken_osty,
            c.fights,
            c.died,
            c.died_after_miss,
            c.fights_with_lethal,
            c.fights_with_miss,
            d.len(),
            p(0.5),
            p(0.9),
            d.last().copied().unwrap_or(0),
        )
    }
}
