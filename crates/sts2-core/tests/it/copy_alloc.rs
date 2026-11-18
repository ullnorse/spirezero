//! `Combat::clone_from` allocates nothing in steady state: a lookahead-shaped loop (every
//! legal action applied to one scratch combat refilled from the real one) is played twice over
//! the same fights, and the second pass's copies must not allocate. A copy that allocates is a
//! derived `clone_from` (`*self = source.clone()`), a list copied without reusing its buffer, or
//! a step that throws away a buffer the copy then has to grow again.
//!
//! The creatures (their powers) and the relics are inline lists (`docs/specs/stage4-design.md`
//! section 10.3): their copies allocate nothing from the first pass on, with no warm-up.

use sts2_core::combat::{Creature, Relics};
use sts2_core::content::encounters::Encounter;
use sts2_core::run::rooms::OVERGROWTH;
use sts2_core::{Action, Character, Combat, CombatConfig, Status};

/// How many allocations (and reallocations) `f` makes on this thread. `allocation-counter`
/// is the global allocator of this test binary: a counting allocator is `unsafe` code, which
/// the workspace forbids.
fn allocations(f: impl FnOnce()) -> u64 {
    allocation_counter::measure(f).count_total
}

/// The bench's W1 start for `seed` (`sts2-bench`'s `fight_start`).
fn fight_start(character: Character, seed: u64) -> Combat {
    let encounters = OVERGROWTH.encounters;
    let info = &encounters[usize::try_from(seed).expect("fits usize") % encounters.len()];
    let encounter = Encounter::from_entry(info.id).expect("every Overgrowth encounter is ported");
    let config = CombatConfig::new(character, &format!("W1S{seed}"), 0)
        .expect("every starter card is ported")
        .with_encounter(encounter);
    Combat::new(config)
}

/// Plays one random fight from `start` (`SplitMix64` from `seed`), copying the fight into `copy`
/// and applying each legal action to it at every step; the allocations of the copies.
fn lookahead_fight(start: &Combat, copy: &mut Combat, seed: u64) -> u64 {
    let mut rng = seed;
    let mut pick = |n: usize| {
        rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        usize::try_from((z ^ (z >> 31)) % u64::try_from(n).expect("fits u64")).expect("fits")
    };
    let mut combat = start.clone();
    let mut legal: Vec<Action> = Vec::new();
    let mut copies = 0;
    for _ in 0..400 {
        if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
            break;
        }
        combat.legal_actions_into(&mut legal);
        for a in &legal {
            copies += allocations(|| copy.clone_from(&combat));
            copy.apply(a.clone()).expect("legal action");
        }
        let action = legal.swap_remove(pick(legal.len()));
        combat.apply(action).expect("legal action");
    }
    copies
}

/// Plays one random fight from `start` as [`lookahead_fight`] does, copying the creatures and
/// the relics into buffers that have never seen this fight; the allocations of those copies.
fn creature_copies_fight(start: &Combat, seed: u64) -> u64 {
    let mut rng = seed;
    let mut combat = start.clone();
    // Room for every summon, so that a longer creature list is not a reallocation.
    let mut creatures: Vec<Creature> = Vec::with_capacity(16);
    creatures.extend(start.state().creatures.iter().cloned());
    let mut relics = Relics::new();
    let mut legal: Vec<Action> = Vec::new();
    let mut copies = 0;
    for _ in 0..400 {
        if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
            break;
        }
        combat.legal_actions_into(&mut legal);
        let state = combat.state();
        for _ in &legal {
            copies += allocations(|| {
                creatures.clone_from(&state.creatures);
                relics.clone_from(&state.player.relics);
            });
        }
        assert_eq!(creatures, state.creatures);
        assert_eq!(relics, state.player.relics);
        rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let pick = usize::try_from((rng >> 33) % u64::try_from(legal.len()).expect("fits u64"))
            .expect("fits usize");
        let action = legal.swap_remove(pick);
        combat.apply(action).expect("legal action");
    }
    copies
}

#[test]
#[cfg_attr(
    feature = "tiny-caps",
    ignore = "capacity 1 spills every list, and a spill is a heap buffer"
)]
fn creature_and_relic_copies_allocate_nothing() {
    for character in [
        Character::Ironclad,
        Character::Silent,
        Character::Necrobinder,
    ] {
        for seed in 0..22 {
            let start = fight_start(character, seed);
            let n = creature_copies_fight(&start, seed);
            assert_eq!(n, 0, "{character:?} seed {seed}: {n} allocations in copies");
        }
    }
}

#[test]
fn clone_from_allocates_nothing_in_steady_state() {
    for character in [
        Character::Ironclad,
        Character::Silent,
        Character::Necrobinder,
    ] {
        for seed in 0..22 {
            let start = fight_start(character, seed);
            let mut copy = start.clone();
            // The first pass grows the copy's buffers to what these fights need.
            for rep in 0..3 {
                lookahead_fight(&start, &mut copy, seed * 8 + rep);
            }
            for rep in 0..3 {
                let n = lookahead_fight(&start, &mut copy, seed * 8 + rep);
                assert_eq!(
                    n, 0,
                    "{character:?} seed {seed} rep {rep}: {n} allocations in copies"
                );
            }
        }
    }
}
