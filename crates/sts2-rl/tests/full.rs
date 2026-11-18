//! The full-view section (RLN30, `sts2_rl::obs_full`): a creature's past moves are the moves it
//! showed on earlier turns (never one to come), hidden state does not reach them, and the
//! section through the run env changes nothing else.

use std::collections::HashMap;

use sts2_core::combat::{CombatState, CreatureId};
use sts2_core::rng::{Rng, RunRngSet};
use sts2_core::{Combat, Status};
use sts2_rl::obs::{CREATURES, OBS_BYTES, Observer};
use sts2_rl::obs_full::{FULL_BYTES, HISTORY, HISTORY_SLOT_BYTES, MOVE_BYTES, move_key};
use sts2_rl::obs_v2::{V2_BYTES, V2Observer};
use sts2_rl::run_obs::{FULL_AT, RUN_OBS_BYTES};
use sts2_rl::{RunConfig, RunEnvs, RunOptions, SeedSource, Start};

/// `SplitMix64`, the policy's RNG.
struct Policy(u64);

impl Policy {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform legal action of `combat`, or `None` when there is none.
    fn act(&mut self, combat: &mut Combat, legal: &mut Vec<sts2_core::combat::Action>) -> bool {
        combat.legal_actions_into(legal);
        if legal.is_empty() {
            return false;
        }
        let count = u64::try_from(legal.len()).expect("fits u64");
        let nth = usize::try_from(self.next() % count).expect("below count");
        combat
            .apply(legal.swap_remove(nth))
            .expect("a listed action applies");
        true
    }
}

/// One fight's observers, kept across its observations (the past moves' intents are recorded).
struct Observers(Observer, V2Observer);

impl Observers {
    fn new() -> Self {
        Self(Observer::new(true), V2Observer::default())
    }

    /// The full-view section of `combat`.
    fn full(&mut self, combat: &Combat) -> Vec<u8> {
        let mut obs = vec![0; OBS_BYTES];
        self.0.observe(combat, &mut obs);
        let mut out = vec![0; V2_BYTES];
        self.1.write(combat, self.0.intent_owners(), &mut out);
        let mut full = vec![0; FULL_BYTES];
        self.1.write_full(combat, &mut full);
        full
    }
}

/// The creature slots of the observation (allies, then enemies).
fn slots(s: &CombatState) -> Vec<CreatureId> {
    s.allies
        .iter()
        .chain(&s.enemies)
        .copied()
        .take(CREATURES)
        .collect()
}

/// A slot's (count, past move keys newest first).
fn history(full: &[u8], slot: usize) -> (u8, Vec<u16>) {
    let at = slot * HISTORY_SLOT_BYTES;
    let keys = (0..HISTORY)
        .map(|k| {
            let m = at + 1 + k * MOVE_BYTES;
            u16::from_le_bytes([full[m], full[m + 1]])
        })
        .take_while(|&k| k != 0)
        .collect();
    (full[at], keys)
}

/// At every player decision of random play, each monster's past moves are the moves it showed
/// at the end of its earlier turns, newest first (the last [`HISTORY`] - 1 the engine's log
/// keeps besides the move shown now), so never a move to come.
#[test]
fn past_moves_are_the_moves_shown_on_earlier_turns() {
    let (mut checked, mut with_history, mut full_rows, mut seen) = (0, 0, 0, 0);
    let mut legal = Vec::new();
    for seed in 0..220 {
        let mut combat = Start::w1_all(seed).combat;
        let mut policy = Policy(seed ^ 0x415);
        // Per creature: the move it showed at the end of each turn so far, oldest first, and
        // whether a forced move ever held it or it was seen dead (such a creature's log has
        // entries it did not show at a turn's end).
        let mut shown: HashMap<CreatureId, Vec<&'static str>> = HashMap::new();
        let mut irregular: HashMap<CreatureId, bool> = HashMap::new();
        let mut last: HashMap<CreatureId, &'static str> = HashMap::new();
        let mut turn = combat.state().player.turn_number;
        let mut observers = Observers::new();
        for _ in 0..300 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let s = combat.state();
            if s.player.turn_number != turn {
                for (&id, &name) in &last {
                    shown.entry(id).or_default().push(name);
                }
                turn = s.player.turn_number;
            }
            let full = observers.full(&combat);
            full_rows += usize::from(full.iter().any(|&b| b != 0));
            last.clear();
            for (slot, id) in slots(s).into_iter().enumerate() {
                let c = s.creature(id);
                let Some(m) = c.monster.as_ref().filter(|_| c.is_alive()) else {
                    // A dead creature shows no intent this turn; one that revives (an
                    // illusion) logs moves it never showed at a turn's end.
                    irregular.insert(id, true);
                    continue;
                };
                *irregular.entry(id).or_default() |= m.machine.forced.is_some();
                last.insert(id, m.next_move_name());
                let (count, keys) = history(&full, slot);
                if irregular[&id] || !s.enemies.contains(&id) {
                    continue;
                }
                let earlier = shown.get(&id).map_or(&[][..], Vec::as_slice);
                let want: Vec<u16> = earlier
                    .iter()
                    .rev()
                    // The log's last entry is the move shown now, so it keeps one fewer past
                    // move unless a forced move holds the creature (skipped here).
                    .take(HISTORY - 1)
                    .map(|n| move_key(n))
                    .collect();
                assert_eq!(
                    keys, want,
                    "seed {seed} turn {turn} slot {slot}: past moves {keys:?}, shown {earlier:?}"
                );
                assert_eq!(usize::from(count), earlier.len(), "seed {seed} turn {turn}");
                checked += 1;
                with_history += usize::from(!keys.is_empty());
                // Every past move of a creature seen from the fight's start was seen shown.
                let at = slot * HISTORY_SLOT_BYTES;
                for k in 0..keys.len() {
                    assert_eq!(
                        full[at + 1 + k * MOVE_BYTES + 2],
                        1,
                        "seed {seed} turn {turn}"
                    );
                    seen += 1;
                }
            }
            if !policy.act(&mut combat, &mut legal) {
                break;
            }
        }
    }
    assert!(
        checked > 5_000 && with_history > 2_000,
        "{checked} checked, {with_history} with history"
    );
    assert!(
        full_rows > 2_000,
        "{full_rows} positions with history bytes"
    );
    assert!(seen > 2_000, "{seen} past moves seen");
}

/// `combat` with what the player cannot see changed: the draw pile reversed and rotated, every
/// RNG stream reseeded from `salt`.
fn perturbed(combat: &Combat, salt: u64) -> Combat {
    let mut combat = combat.clone();
    combat.edit(|s| {
        let draw = &mut s.player.draw;
        draw.reverse();
        if draw.len() > 2 {
            let by = usize::try_from(salt).expect("a 64-bit usize") % draw.len();
            draw.rotate_left(by);
        }
        s.rng = RunRngSet::new(&format!("FULL{salt}"));
        for (i, creature) in s.creatures.iter_mut().enumerate() {
            if let Some(monster) = &mut creature.monster {
                monster.rng = Rng::new(salt ^ (0xF011 << 16) ^ u64::try_from(i).expect("fits"));
            }
        }
    });
    combat
}

/// Reordering the draw pile and reseeding every stream at every position leaves the section
/// unchanged: observers that only ever see perturbed copies write the same bytes as observers of
/// the real fight.
#[test]
fn hidden_state_does_not_reach_the_past_moves() {
    let mut positions = 0;
    let mut legal = Vec::new();
    for seed in 0..110 {
        let mut combat = Start::w1_all(seed).combat;
        let mut policy = Policy(seed << 3);
        let mut real = Observers::new();
        let mut others = [Observers::new(), Observers::new()];
        for step in 0..120 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let base = real.full(&combat);
            for (salt, other) in [step, step ^ 0x5A5A].into_iter().zip(&mut others) {
                assert!(
                    base == other.full(&perturbed(&combat, salt)),
                    "seed {seed} step {step}: the full-view section changed with hidden state"
                );
            }
            positions += 1;
            if !policy.act(&mut combat, &mut legal) {
                break;
            }
        }
    }
    assert!(positions > 2_000, "{positions} positions");
}

fn envs(obs_full: bool) -> RunEnvs {
    RunEnvs::new(
        4,
        SeedSource::Range {
            start: 0,
            end: 1_000,
            cycle: false,
        },
        RunConfig {
            options: RunOptions {
                obs_v2: true,
                plan_column: true,
                obs_full,
                priv_obs: true,
                ..RunOptions::E2E
            },
            ..RunConfig::default()
        },
    )
}

/// The first set bit of every env's mask.
fn first_actions(e: &RunEnvs) -> Vec<u16> {
    (0..e.len())
        .map(|i| {
            let mask = e.mask(i);
            let word = mask.iter().position(|&w| w != 0).unwrap_or(0);
            u16::try_from(word * 64 + mask[word].trailing_zeros() as usize).expect("an index")
        })
        .collect()
}

#[test]
fn the_full_section_changes_nothing_else() {
    let (mut off, mut on) = (envs(false), envs(true));
    let mut written = 0;
    for step in 0..3_000 {
        for (a, b) in off
            .obs()
            .chunks(RUN_OBS_BYTES)
            .zip(on.obs().chunks(RUN_OBS_BYTES))
        {
            assert_eq!(a[..FULL_AT], b[..FULL_AT], "step {step}");
            assert!(a[FULL_AT..].iter().all(|&x| x == 0), "off is zero");
            written += usize::from(b[FULL_AT..].iter().any(|&x| x != 0));
        }
        assert_eq!(off.masks(), on.masks(), "step {step}");
        assert_eq!(off.priv_obs(), on.priv_obs(), "step {step}");
        let actions = first_actions(&off);
        let a = off.step(&actions).expect("legal");
        let b = on.step(&actions).expect("legal");
        assert_eq!(a, b, "step {step}");
    }
    assert!(
        written > 500,
        "{written} observations with a full-view section"
    );
}

/// The full view through the run env on random A10 runs: no panic, and the intents it records
/// come from the intent cache, whose debug builds recompute every hit and assert it is the same
/// ([`sts2_rl::intents`]'s oracle).
#[test]
fn cached_past_moves_match_a_recompute() {
    let mut e = RunEnvs::new(
        8,
        SeedSource::Range {
            start: 23_000_000,
            end: 23_100_000,
            cycle: false,
        },
        RunConfig {
            ascension: 10,
            options: RunOptions {
                obs_v2: true,
                obs_full: true,
                ..RunOptions::E2E
            },
            ..RunConfig::default()
        },
    );
    let mut policy = Policy(30);
    let mut written = 0;
    for _ in 0..3_000 {
        written += e
            .obs()
            .chunks(RUN_OBS_BYTES)
            .filter(|row| row[FULL_AT..].iter().any(|&x| x != 0))
            .count();
        let actions: Vec<u16> = (0..e.len())
            .map(|i| {
                let mask = e.mask(i);
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                if count == 0 {
                    return 0;
                }
                let nth = u32::try_from(policy.next() % u64::from(count)).expect("below count");
                let index = sts2_rl::action::nth_set(mask, nth).expect("a set bit");
                u16::try_from(index).expect("an index")
            })
            .collect();
        e.step(&actions).expect("legal");
    }
    assert!(written > 1_000, "{written} observations with past moves");
}
