//! TMA1: `RollingBoulderPower`'s two `TestMode` branches (`Models.Powers/RollingBoulderPower.cs`,
//! `Nodes.Vfx/NRollingBoulderVfx.cs`). Test mode: one batched `CreatureCmd.Damage` on all hittable
//! enemies. Real game: the VFX's `HitCreature` makes one `DoDamage` call per enemy, so one
//! enemy's death and after-hooks run before the next enemy is hit.

use super::*;

const BOULDER: i32 = 20;

/// `KaiserCrabBoss` (`Crusher` first, then `Rocket`), the Crusher at 5 HP and the Rocket at 100,
/// both without block, with a Rolling Boulder of `BOULDER` on the player.
fn crab_fight(test_mode: bool) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0).with_encounter(Encounter::KaiserCrabBoss);
    config.test_mode = test_mode;
    let mut c = Combat::new(config);
    let p = player(&c);
    for (i, hp) in [5, 100].into_iter().enumerate() {
        let e = c.state.enemies[i];
        let creature = c.state.creature_mut(e);
        creature.hp = hp;
        creature.max_hp = hp.max(creature.max_hp);
        creature.block = 0;
    }
    with_engine(&mut c, |eng| {
        eng.apply_power(
            p,
            PowerKind::RollingBoulder,
            Decimal::from(BOULDER),
            Some(p),
        )
    });
    c
}

fn turn_start(c: &mut Combat) {
    with_engine(c, |eng: &mut Engine<'_>| -> Flow {
        eng.after_player_turn_start()
    });
}

#[test]
fn test_mode_batches_the_boulder_so_the_second_arm_is_hit_before_crab_rage() {
    // One `Damage` over both arms: the Rocket takes the full hit, then the Crusher's death gives
    // it Crab Rage's 99 block (after the damage).
    let mut c = crab_fight(true);
    let (crusher, rocket) = (c.state.enemies[0], c.state.enemies[1]);
    turn_start(&mut c);
    assert!(c.state.creature(crusher).is_dead());
    let rocket = c.state.creature(rocket);
    assert_eq!(rocket.hp, 100 - BOULDER);
    assert_eq!(rocket.block, 99);
    assert_eq!(rocket.power_amount(PowerKind::Strength), 6);
}

#[test]
fn real_game_hits_one_enemy_at_a_time_so_crab_rage_blocks_the_next_hit() {
    // The first hit kills the Crusher; its death runs `CrabRage.AfterDeath` on the Rocket (6
    // Strength, 99 block) before the boulder reaches the Rocket, so the block eats the hit.
    let mut c = crab_fight(false);
    let (crusher, rocket) = (c.state.enemies[0], c.state.enemies[1]);
    turn_start(&mut c);
    assert!(c.state.creature(crusher).is_dead());
    let rocket = c.state.creature(rocket);
    assert_eq!(rocket.hp, 100);
    assert_eq!(rocket.block, 99 - BOULDER);
    assert_eq!(rocket.power_amount(PowerKind::Strength), 6);
    // `SetAmount(Amount + 5)` in both branches.
    let amount = c
        .state
        .creature(player(&c))
        .power_amount(PowerKind::RollingBoulder);
    assert_eq!(amount, BOULDER + 5);
}

#[test]
fn both_branches_agree_without_after_hooks_between_the_hits() {
    // Two plain enemies' worth of damage on a lone target is the same either way.
    for test_mode in [true, false] {
        let mut c = crab_fight(test_mode);
        let rocket = c.state.enemies[1];
        c.state.creature_mut(c.state.enemies[0]).hp = 100;
        turn_start(&mut c);
        assert_eq!(c.state.creature(rocket).hp, 100 - BOULDER, "{test_mode}");
    }
}
