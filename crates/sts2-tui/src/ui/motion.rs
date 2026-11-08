//! Motion: what a fight's frames animate for a moment after something changes, then rest
//! exactly as they are drawn without it.
//!
//! Each frame of a fight reports its creatures' HP and Block ([`observe`]); a loss starts an
//! animation for that creature: its bar keeps the lost cells in the preview's tint for a beat,
//! then drains them, and the HP (or Block) it lost floats beside its name. The enemies' turn
//! plays out in steps, one hit after another with the attacker lit ([`acting`]), under an
//! `Enemy turn` banner, then `Your turn`. The renderers ask what to draw ([`ghost`], [`hit`],
//! [`acting`], [`banner`]); the event loop redraws on a tick while [`active`], and any key
//! finishes everything at once ([`finish`]).
//!
//! Display state only, a thread-local like the theme: it never reaches the engine, a replay or
//! a save. It is off until the event loop turns it on ([`set_enabled`], from the settings), so
//! the tests draw every frame as it rests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long the lost cells stay before they drain, and how long the drain takes.
const HOLD: Duration = Duration::from_millis(250);
const DRAIN: Duration = Duration::from_millis(350);
/// How long a hit's number floats beside the name.
const FLOAT: Duration = Duration::from_millis(900);
/// How long a new round's banner shows.
const BANNER: Duration = Duration::from_millis(700);
/// How long each step of the enemies' turn takes, and how long of it its hit drains.
const STEP: Duration = Duration::from_millis(500);
const STEP_DRAIN: Duration = Duration::from_millis(300);
/// The event loop's redraw tick while something moves.
pub const TICK: Duration = Duration::from_millis(50);

/// A creature's HP and Block as a frame drew them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
    pub id: u32,
    pub hp: i32,
    pub block: i32,
}

/// One hit of the enemies' turn, in the order it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The creature that acted (lit while the step plays).
    pub actor: Option<u32>,
    pub receiver: u32,
    pub hp_lost: i32,
    pub blocked: i32,
}

/// The enemies' turn playing out: its steps from the HP the creatures had before it.
#[derive(Clone, Debug)]
struct Turn {
    steps: Vec<Step>,
    before: HashMap<u32, i32>,
    started: Instant,
}

impl Turn {
    fn length(&self) -> Duration {
        STEP * u32::try_from(self.steps.len()).unwrap_or(u32::MAX)
    }

    /// The step playing at `at`, and how far into it.
    fn step(&self, at: Instant) -> Option<(usize, Duration)> {
        let t = at.duration_since(self.started);
        let i = usize::try_from(t.as_millis() / STEP.as_millis()).ok()?;
        (i < self.steps.len()).then(|| {
            (
                i,
                t.checked_sub(STEP * u32::try_from(i).unwrap_or(0))
                    .unwrap_or_default(),
            )
        })
    }

    /// The HP creature `id`'s bar shows at `at`: what it had before the turn, less every hit it
    /// took up to the playing one, that one draining.
    fn shown(&self, id: u32, at: Instant) -> Option<i32> {
        let before = *self.before.get(&id)?;
        let (now, into) = self.step(at)?;
        let lost = |steps: &[Step]| -> i32 {
            steps
                .iter()
                .filter(|s| s.receiver == id)
                .map(|s| s.hp_lost)
                .sum()
        };
        let done = lost(&self.steps[..now]);
        let this = lost(&self.steps[now..=now]);
        let left = STEP_DRAIN.saturating_sub(into).as_millis();
        let draining = i128::from(this) * i128::try_from(left).unwrap_or(0)
            / i128::try_from(STEP_DRAIN.as_millis()).unwrap_or(1);
        Some(before - done - this + i32::try_from(draining).unwrap_or(0))
    }
}

/// A creature's running animation.
#[derive(Clone, Copy, Debug)]
struct Hit {
    /// The HP before the loss, drained down to the HP now.
    from: i32,
    to: i32,
    /// What floats beside the name: the HP lost, else the Block lost.
    hp_lost: i32,
    block_lost: i32,
    started: Instant,
}

#[derive(Default)]
struct State {
    enabled: bool,
    /// The round and the creatures the last frame drew, to compare the next frame with.
    round: Option<u32>,
    seen: HashMap<u32, Seen>,
    hits: HashMap<u32, Hit>,
    turn: Option<Turn>,
    banner: Option<(&'static str, Instant)>,
    /// The clock the tests set; `None` reads the real one.
    now: Option<Instant>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn now(state: &State) -> Instant {
    state.now.unwrap_or_else(Instant::now)
}

/// Turns motion on or off (the `Animations` setting); off drops whatever runs.
pub fn set_enabled(on: bool) {
    STATE.with_borrow_mut(|s| {
        if s.enabled != on {
            *s = State {
                enabled: on,
                now: s.now,
                ..State::default()
            };
        }
    });
}

/// Sets the clock the next frames read (tests step an animation by hand).
pub fn set_now(at: Instant) {
    STATE.with_borrow_mut(|s| s.now = Some(at));
}

/// Takes a fight's frame: its round, its creatures, and the hits of the enemies' turn that
/// came before this round (empty when none).
///
/// A loss since the last frame starts that creature's animation; a later round plays the
/// enemies' turn out from `turn` (then the banner); a new fight (other creatures, an earlier
/// round) only sets the baseline.
pub fn observe(round: u32, creatures: &[Seen], turn: &[Step]) {
    STATE.with_borrow_mut(|s| {
        if !s.enabled {
            return;
        }
        let at = now(s);
        let same_fight = s.round.is_some_and(|r| r <= round)
            && creatures.iter().all(|c| s.seen.contains_key(&c.id));
        if same_fight {
            let new_round = s.round.is_some_and(|r| r < round);
            if new_round && !turn.is_empty() {
                s.hits.clear();
                s.turn = Some(Turn {
                    steps: turn.to_vec(),
                    before: s.seen.values().map(|c| (c.id, c.hp)).collect(),
                    started: at,
                });
            } else if new_round {
                s.banner = Some(("Your turn", at));
            }
            for c in creatures.iter().filter(|_| s.turn.is_none()) {
                let Some(before) = s.seen.get(&c.id) else {
                    continue;
                };
                let hp_lost = before.hp - c.hp;
                let block_lost = before.block - c.block;
                if hp_lost > 0 || (block_lost > 0 && c.hp == before.hp) {
                    // A hit during a drain drains from where the bar shows now.
                    let from = s.hits.get(&c.id).map_or(before.hp, |h| shown(h, at));
                    s.hits.insert(
                        c.id,
                        Hit {
                            from,
                            to: c.hp,
                            hp_lost: hp_lost.max(0),
                            block_lost: block_lost.max(0),
                            started: at,
                        },
                    );
                }
            }
        } else {
            s.hits.clear();
            s.turn = None;
            s.banner = None;
        }
        // The enemies' turn played out: `Your turn` for a moment.
        if s.turn
            .as_ref()
            .is_some_and(|t| at.duration_since(t.started) >= t.length())
        {
            s.turn = None;
            s.banner = Some(("Your turn", at));
        }
        s.round = Some(round);
        s.seen = creatures.iter().map(|c| (c.id, *c)).collect();
        s.hits
            .retain(|_, h| at.duration_since(h.started) < FLOAT.max(HOLD + DRAIN));
        if s.banner
            .is_some_and(|(_, started)| at.duration_since(started) >= BANNER)
        {
            s.banner = None;
        }
    });
}

/// The HP a draining bar shows at `at`: `from` through the hold, then down to `to`.
fn shown(hit: &Hit, at: Instant) -> i32 {
    let Some(draining) = at.duration_since(hit.started).checked_sub(HOLD) else {
        return hit.from;
    };
    let done = draining.as_millis().min(DRAIN.as_millis());
    let left = i128::from(hit.from - hit.to)
        * i128::try_from(DRAIN.as_millis() - done).unwrap_or(0)
        / i128::try_from(DRAIN.as_millis()).unwrap_or(1);
    hit.to + i32::try_from(left).unwrap_or(0)
}

/// The HP creature `id`'s bar still shows above its HP (the lost cells, in the preview's tint),
/// while it drains.
#[must_use]
pub fn ghost(id: u32) -> Option<i32> {
    STATE.with_borrow(|s| {
        let at = now(s);
        if let Some(turn) = &s.turn {
            let hp = s.seen.get(&id)?.hp;
            return turn.shown(id, at).filter(|&shown| shown > hp);
        }
        let hit = s.hits.get(&id)?;
        let hp = shown(hit, now(s));
        (hp > hit.to).then_some(hp)
    })
}

/// What floats beside creature `id`'s name while its hit is fresh: `−6`, or `▣ −5` when only
/// Block was lost.
#[must_use]
pub fn hit(id: u32) -> Option<String> {
    STATE.with_borrow(|s| {
        if let Some(turn) = &s.turn {
            let (i, _) = turn.step(now(s))?;
            let step = turn.steps[i];
            return (step.receiver == id).then(|| {
                if step.hp_lost > 0 {
                    format!("−{}", step.hp_lost)
                } else {
                    format!("▣ −{}", step.blocked)
                }
            });
        }
        let hit = s.hits.get(&id)?;
        if now(s).duration_since(hit.started) >= FLOAT {
            return None;
        }
        if hit.hp_lost > 0 {
            Some(format!("−{}", hit.hp_lost))
        } else {
            Some(format!("▣ −{}", hit.block_lost))
        }
    })
}

/// Whether creature `id` acts in the step of the enemies' turn playing now (its name lit).
#[must_use]
pub fn acting(id: u32) -> bool {
    STATE.with_borrow(|s| {
        s.turn.as_ref().is_some_and(|t| {
            t.step(now(s))
                .is_some_and(|(i, _)| t.steps[i].actor == Some(id))
        })
    })
}

/// The banner over the fight, while it shows.
#[must_use]
pub fn banner() -> Option<&'static str> {
    STATE.with_borrow(|s| {
        if s.turn.is_some() {
            return Some("Enemy turn");
        }
        s.banner
            .filter(|(_, started)| now(s).duration_since(*started) < BANNER)
            .map(|(text, _)| text)
    })
}

/// Whether anything still moves, so the event loop redraws on [`TICK`].
#[must_use]
pub fn active() -> bool {
    STATE.with_borrow(|s| {
        let at = now(s);
        s.turn.is_some()
            || s.hits
                .values()
                .any(|h| at.duration_since(h.started) < FLOAT.max(HOLD + DRAIN))
            || s.banner
                .is_some_and(|(_, started)| at.duration_since(started) < BANNER)
    })
}

/// Ends every animation at once (a key was pressed): the next frame rests.
pub fn finish() {
    STATE.with_borrow_mut(|s| {
        s.hits.clear();
        s.turn = None;
        s.banner = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slime(hp: i32, block: i32) -> [Seen; 1] {
        [Seen { id: 7, hp, block }]
    }

    #[test]
    fn a_hit_holds_drains_then_rests() {
        let start = Instant::now();
        set_now(start);
        set_enabled(true);
        observe(1, &slime(20, 0), &[]);
        assert_eq!(ghost(7), None, "the first frame only sets the baseline");
        observe(1, &slime(14, 0), &[]);
        assert_eq!(ghost(7), Some(20), "held at the HP before the hit");
        assert_eq!(hit(7).as_deref(), Some("−6"));
        assert!(active());
        set_now(start + HOLD + DRAIN / 2);
        assert_eq!(ghost(7), Some(17), "halfway down");
        set_now(start + HOLD + DRAIN);
        assert_eq!(ghost(7), None, "drained");
        set_now(start + FLOAT);
        observe(1, &slime(14, 0), &[]);
        assert_eq!(hit(7), None);
        assert!(!active(), "everything rests");
        set_enabled(false);
    }

    #[test]
    fn block_alone_floats_and_a_new_round_shows_the_banner() {
        let start = Instant::now();
        set_now(start);
        set_enabled(true);
        observe(1, &slime(20, 8), &[]);
        observe(1, &slime(20, 3), &[]);
        assert_eq!(hit(7).as_deref(), Some("▣ −5"));
        assert_eq!(ghost(7), None, "no HP lost, nothing to drain");
        observe(2, &slime(20, 0), &[]);
        assert_eq!(banner(), Some("Your turn"));
        finish();
        assert_eq!(banner(), None);
        assert!(!active());
        set_enabled(false);
    }

    #[test]
    fn the_enemies_turn_plays_out_hit_by_hit() {
        let start = Instant::now();
        set_now(start);
        set_enabled(true);
        let you = |hp| Seen {
            id: 1,
            hp,
            block: 0,
        };
        let them = Seen {
            id: 7,
            hp: 20,
            block: 0,
        };
        observe(1, &[you(50), them], &[]);
        let steps = [
            Step {
                actor: Some(7),
                receiver: 1,
                hp_lost: 6,
                blocked: 0,
            },
            Step {
                actor: Some(7),
                receiver: 1,
                hp_lost: 4,
                blocked: 0,
            },
        ];
        observe(2, &[you(40), them], &steps);
        assert_eq!(banner(), Some("Enemy turn"));
        assert!(acting(7));
        assert_eq!(
            ghost(1),
            Some(50),
            "the first hit starts from the HP before the turn"
        );
        assert_eq!(hit(1).as_deref(), Some("−6"));
        set_now(start + STEP);
        assert_eq!(
            ghost(1),
            Some(44),
            "the first hit landed, the second starts draining"
        );
        assert_eq!(hit(1).as_deref(), Some("−4"));
        set_now(start + STEP + STEP_DRAIN);
        assert_eq!(ghost(1), None, "the second hit drained to the HP now");
        set_now(start + STEP * 2);
        observe(2, &[you(40), them], &steps);
        assert_eq!(banner(), Some("Your turn"));
        assert!(!acting(7));
        assert_eq!(ghost(1), None);
        set_enabled(false);
    }

    #[test]
    fn off_or_a_new_fight_animates_nothing() {
        set_enabled(false);
        observe(1, &slime(20, 0), &[]);
        observe(1, &slime(10, 0), &[]);
        assert_eq!(ghost(7), None);
        set_enabled(true);
        observe(3, &slime(20, 0), &[]);
        observe(
            1,
            &[Seen {
                id: 9,
                hp: 5,
                block: 0,
            }],
            &[],
        );
        assert_eq!(ghost(9), None, "another fight's creatures");
        assert_eq!(banner(), None);
        set_enabled(false);
    }
}
