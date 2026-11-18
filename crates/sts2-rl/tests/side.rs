//! RLN21: the v2 section's facing and shown numbers (`sts2_rl::obs_v2`, `SIDE_AT`,
//! `DISPLAY_AT`) on the fights that have them: Kaiser Crab (`SurroundedPower`'s facing and the
//! arm behind), Bygone Effigy (`SlowPower`'s bonus) and Skulking Colony (`HardenedShellPower`'s
//! HP loss left). At every position of random play the bytes agree with the engine (the arm
//! marked behind is the one whose hits the damage hooks scale up) and do not change with hidden
//! state (the draw pile's order, every RNG stream).

use sts2_core::combat::{CombatState, CreatureId, Encounter};
use sts2_core::content::powers::PowerKind;
use sts2_core::rng::{Rng, RunRngSet};
use sts2_core::{Combat, CombatConfig, Decimal, Status};
use sts2_rl::obs::{CREATURES, OBS_BYTES, Observer, POWERS};
use sts2_rl::obs_v2::{DISPLAY_AT, DISPLAY_BYTES, SIDE_AT, SIDE_BYTES, V2_BYTES, V2Observer};

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
}

fn perturb(s: &mut CombatState, salt: u64) {
    s.player.draw.reverse();
    s.rng = RunRngSet::new(&format!("SIDE{salt}"));
    for (i, creature) in s.creatures.iter_mut().enumerate() {
        if let Some(monster) = &mut creature.monster {
            monster.rng = Rng::new(salt ^ u64::try_from(i).expect("fits"));
        }
    }
}

fn observe_v2(combat: &Combat) -> Vec<u8> {
    let mut obs = vec![0; OBS_BYTES];
    let mut observer = Observer::new(true);
    observer.observe(combat, &mut obs);
    let mut out = vec![0; V2_BYTES];
    V2Observer::default().write(combat, observer.intent_owners(), &mut out);
    out
}

/// The creature of each observation slot (allies, then enemies).
fn slots(s: &CombatState) -> Vec<CreatureId> {
    s.allies
        .iter()
        .chain(&s.enemies)
        .copied()
        .take(CREATURES)
        .collect()
}

/// How much 1,000 more base damage from `arm` adds on the player: the multipliers alone.
fn slope(combat: &Combat, arm: CreatureId) -> i32 {
    let h = combat.hooked();
    h.intent_damage(arm, Decimal::from(2000)) - h.intent_damage(arm, Decimal::from(1000))
}

#[derive(Default)]
struct Seen {
    positions: usize,
    facing: [usize; 3],
    behind_checked: usize,
    shown: usize,
}

fn play(encounter: Encounter, seeds: u64, seen: &mut Seen) {
    let mut legal = Vec::new();
    for seed in 0..seeds {
        let config = CombatConfig::necrobinder(&format!("RLN21-{seed}"), 10);
        let mut combat = Combat::new(config.with_encounter(encounter));
        let mut policy = Policy(seed ^ 0x51DE);
        for step in 0..150 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let v2 = observe_v2(&combat);
            let mut other = combat.clone();
            other.edit(|s| perturb(s, step));
            assert!(
                v2 == observe_v2(&other),
                "{encounter:?} seed {seed} step {step}: the side bytes changed with hidden state"
            );
            let s = combat.state();
            let ids = slots(s);
            seen.facing[usize::from(v2[SIDE_AT])] += 1;
            let arms: Vec<(CreatureId, u8)> = ids
                .iter()
                .enumerate()
                .filter(|&(_, &id)| s.creature(id).is_alive())
                .map(|(slot, &id)| (id, v2[SIDE_AT + slot * SIDE_BYTES + 1]))
                .filter(|&(_, arm)| arm > 0)
                .collect();
            if let [(a, a_side), (b, b_side)] = arms[..] {
                assert_ne!(a_side, b_side, "one arm behind, one in front");
                let (behind, front) = if a_side == 1 { (a, b) } else { (b, a) };
                assert!(
                    slope(&combat, behind) > slope(&combat, front),
                    "{encounter:?} seed {seed} step {step}: the arm marked behind is not the one hitting harder"
                );
                seen.behind_checked += 1;
            }
            for (slot, &id) in ids.iter().enumerate() {
                for (j, power) in s.creature(id).powers.iter().take(POWERS).enumerate() {
                    let at = DISPLAY_AT + (slot * POWERS + j) * DISPLAY_BYTES;
                    let expect = matches!(power.kind, PowerKind::Slow | PowerKind::HardenedShell);
                    assert_eq!(v2[at] == 1, expect, "{:?} shown flag", power.kind);
                    seen.shown += usize::from(expect && v2[at + 1..at + 3] != [0, 0]);
                }
            }
            seen.positions += 1;
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            let count = u64::try_from(legal.len()).expect("fits u64");
            let nth = usize::try_from(policy.next() % count).expect("below count");
            combat
                .apply(legal.swap_remove(nth))
                .expect("a listed action applies");
        }
    }
}

#[test]
fn the_crab_facing_is_the_engines_and_turns() {
    let mut seen = Seen::default();
    play(Encounter::KaiserCrabBoss, 200, &mut seen);
    assert!(seen.positions > 1_000, "{} positions", seen.positions);
    assert_eq!(seen.facing[0], 0, "the player is always Surrounded");
    assert!(
        seen.facing[1] > 100 && seen.facing[2] > 100,
        "facing right {} and left {}",
        seen.facing[1],
        seen.facing[2]
    );
    assert!(seen.behind_checked > 500, "{} checked", seen.behind_checked);
}

#[test]
fn slow_and_hardened_shell_show_their_numbers() {
    for encounter in [Encounter::BygoneEffigyElite, Encounter::SkulkingColonyElite] {
        let mut seen = Seen::default();
        play(encounter, 20, &mut seen);
        assert!(
            seen.positions > 200,
            "{encounter:?}: {} positions",
            seen.positions
        );
        assert_eq!(
            seen.facing[1] + seen.facing[2],
            0,
            "{encounter:?}: no facing"
        );
        assert!(
            seen.shown > 100,
            "{encounter:?}: {} shown numbers",
            seen.shown
        );
    }
}
