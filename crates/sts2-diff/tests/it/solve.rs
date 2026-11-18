//! The exhaustive fight solver (`docs/specs/solver.md`): merging states and chance outcomes
//! (and the sorted draw runs that come with merging) gives the brute-force numbers, and the
//! solver's policy played in the unscripted engine loses the HP the solver expects.
#![allow(
    clippy::float_arithmetic,
    clippy::suboptimal_flops,
    reason = "the solver's values are f64 probabilities"
)]

use sts2_core::Character;
use sts2_core::Combat;
use sts2_core::Status;
use sts2_core::combat::{Action, Encounter};
use sts2_core::content::cards::CardKind;
use sts2_diff::solve::{self, Config, Setup, Solver, Value};

/// Ironclad without Burning Blood (no healing: heal bound 0) against a weakened Fuzzy Wurm
/// Crawler, played into turn 2: Bash and Strike, end turn, so the turn-3 hand comes from a
/// reshuffle.
fn small_fight(enemy_hp: i32, player_hp: i32) -> Combat {
    let mut setup = Setup::new(Character::Ironclad, Encounter::FuzzyWurmCrawlerWeak);
    setup.starter_relic = false;
    setup.enemy_hp = Some(enemy_hp);
    setup.player_hp = Some(player_hp);
    setup.hand = vec![
        CardKind::StrikeIronclad,
        CardKind::StrikeIronclad,
        CardKind::DefendIronclad,
        CardKind::DefendIronclad,
        CardKind::Bash,
    ];
    let mut combat = setup.start().expect("the fight starts");
    solve::play_kind(&mut combat, CardKind::Bash).expect("Bash");
    solve::play_kind(&mut combat, CardKind::StrikeIronclad).expect("Strike");
    assert_eq!(combat.apply(Action::EndTurn), Ok(Status::PlayerTurn));
    combat
}

fn config(merge: bool, max_turns: u32) -> Config {
    Config {
        merge,
        max_turns,
        heal_bound: 0,
        ..Config::default()
    }
}

fn close(a: &Value, b: &Value) -> bool {
    (a.hp_lo - b.hp_lo).abs() < 1e-9
        && (a.hp_hi - b.hp_hi).abs() < 1e-9
        && (a.death_lo - b.death_lo).abs() < 1e-9
        && (a.death_hi - b.death_hi).abs() < 1e-9
}

#[test]
fn solve_merging_equals_brute_force() {
    // Turn 2 is drawn whole (the five cards left); the end of turn 2 reshuffles, so turn 3 is
    // chance. The cap stops at turn 3: bounds for longer lines, exact values for the rest.
    let combat = small_fight(30, 7);
    let mut merged = Solver::new(config(true, 3));
    let mut brute = Solver::new(config(false, 3));
    let a = merged.action_values(&combat).expect("merged solve");
    let b = brute.action_values(&combat).expect("brute-force solve");
    assert_eq!(a.len(), b.len());
    let mut chance_seen = false;
    for ((action, va), (_, vb)) in a.iter().zip(&b) {
        assert!(
            close(va, vb),
            "{action:?}: merged {va:?} != brute force {vb:?}"
        );
        chance_seen |= va.death_hi > 0.0 && va.death_lo < 1.0;
    }
    assert!(chance_seen, "the fight has a chance outcome: {a:?}");
    assert!(
        brute.stats.states > 2 * merged.stats.states,
        "merging merges: {:?} vs {:?}",
        merged.stats,
        brute.stats
    );
}

#[test]
fn solve_monte_carlo_matches_expectation() {
    // A Strike-only opening hand: the Wurm's first Acid Goop lands, and the rest depends on the
    // draws (expected loss about 4.52).
    let mut setup = Setup::new(Character::Ironclad, Encounter::FuzzyWurmCrawlerWeak);
    setup.starter_relic = false;
    setup.enemy_hp = Some(50);
    setup.hand = vec![CardKind::StrikeIronclad; 5];
    let combat = setup.start().expect("the fight starts");
    let mut solver = Solver::new(config(true, 40));
    let root = solver.value(&combat).expect("solve");
    assert!(root.is_exact(), "{root:?}");
    let hp0 = f64::from(solve::player_hp(combat.state()));
    let expected = hp0 - root.hp();
    let n = 1000;
    let losses: Vec<f64> = (0..n)
        .map(|seed| hp0 - f64::from(solve::play_policy(&mut solver, &combat, seed).expect("play")))
        .collect();
    let count = f64::from(u32::try_from(n).expect("small"));
    let mean = losses.iter().sum::<f64>() / count;
    let var = losses.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / (count - 1.0);
    let half = 3.29 * (var / count).sqrt() + 1e-9;
    assert!(var > 0.0, "the fight is random under the policy");
    assert!(
        (mean - expected).abs() <= half,
        "Monte Carlo mean {mean} vs solver {expected} (99.9% half-width {half})"
    );
}

/// Two enemies: their rolls and targets multiply the chance nodes and the actions. Ironclad
/// without Burning Blood at 12 HP against two Corpse Slugs at 8 HP, capped at turn 2 (the slugs' rolls
/// for turn 2 are the chance).
#[test]
fn solve_two_enemies_merging_equals_brute_force() {
    let mut setup = Setup::new(Character::Ironclad, Encounter::CorpseSlugsWeak);
    setup.starter_relic = false;
    setup.enemy_hp = Some(8);
    setup.player_hp = Some(12);
    let combat = setup.start().expect("the fight starts");
    let mut merged = Solver::new(config(true, 2));
    let mut brute = Solver::new(config(false, 2));
    let a = merged.action_values(&combat).expect("merged solve");
    let b = brute.action_values(&combat).expect("brute-force solve");
    assert_eq!(a.len(), b.len());
    assert!(a.len() > 3, "targets multiply the actions: {a:?}");
    for ((action, va), (_, vb)) in a.iter().zip(&b) {
        assert!(
            close(va, vb),
            "{action:?}: merged {va:?} != brute force {vb:?}"
        );
    }
}

/// A table too small for the fight evicts entries instead of cutting the search: the values
/// are the same, only the work grows.
#[test]
fn solve_evicting_table_keeps_values() {
    let combat = small_fight(30, 7);
    let mut roomy = Solver::new(config(true, 4));
    let mut tight = Solver::new(Config {
        max_states: 64,
        ..config(true, 4)
    });
    let a = roomy.action_values(&combat).expect("roomy solve");
    let b = tight.action_values(&combat).expect("tight solve");
    for ((action, va), (_, vb)) in a.iter().zip(&b) {
        assert!(close(va, vb), "{action:?}: {va:?} != {vb:?} with eviction");
    }
    assert!(
        tight.stats.evicted > 0,
        "the table filled: {:?}",
        tight.stats
    );
}

/// Burning Blood's heal bound, held to the fight's end, prunes without changing a value: the
/// same fight solved with no bound at all gives the same numbers.
#[test]
fn solve_end_heal_bound_keeps_values() {
    let mut setup = Setup::new(Character::Ironclad, Encounter::FuzzyWurmCrawlerWeak);
    setup.enemy_hp = Some(24);
    setup.player_hp = Some(20);
    setup.hand = vec![CardKind::StrikeIronclad; 5];
    let combat = setup.start().expect("the fight starts");
    let heal = solve::heal_at_end(combat.state());
    assert_eq!(heal, 6, "Burning Blood heals 6");
    let mut bounded = Solver::new(Config {
        heal_bound: heal,
        end_heal_only: true,
        ..config(true, 40)
    });
    let mut open = Solver::new(Config {
        heal_bound: i32::MAX / 2,
        ..config(true, 40)
    });
    let a = bounded.action_values(&combat).expect("bounded solve");
    let b = open.action_values(&combat).expect("unbounded solve");
    for ((action, va), (_, vb)) in a.iter().zip(&b) {
        assert!(va.is_exact(), "{action:?}: {va:?}");
        assert!(
            close(va, vb),
            "{action:?}: {va:?} != {vb:?} without a bound"
        );
    }
}
