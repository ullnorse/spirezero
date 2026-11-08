//! The last Underdocks monsters (PU2): the gremlin heist, the Matriarch's sleep and the rats'
//! summons, and the two core fixes behind them.

use super::*;
use crate::content::monsters::{FatGremlin, MonsterModel as _, TwoTailedRat};
use crate::content::powers::thievery;
use crate::run::odds::GoldProportion;

fn facing(encounter: Encounter) -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(encounter))
}

fn kind_of(c: &Combat, id: CreatureId) -> MonsterKind {
    c.state
        .creature(id)
        .monster
        .as_ref()
        .expect("a monster")
        .kind
}

fn find(c: &Combat, pred: impl Fn(MonsterKind) -> bool) -> Option<CreatureId> {
    c.state
        .enemies
        .iter()
        .copied()
        .find(|&e| pred(kind_of(c, e)))
}

#[test]
fn a_created_enemy_takes_powers_and_has_no_move_until_rolled() {
    // PowerCmd.cs:98-107, Creature.cs:309-322: `CanReceivePowers` is `CombatState != null`, which
    // `CreateCreature` sets; `NextMove` is `UNSET_MOVE` until the first `RollMove`.
    let mut c = facing(Encounter::FogmogNormal);
    let fat = with_engine(&mut c, |eng| {
        let fat = eng.create_enemy(MonsterKind::FatGremlin(FatGremlin), Some("fat"));
        eng.apply_power(fat, PowerKind::Strength, Decimal::from(2), None)?;
        Ok(fat)
    });
    assert!(!c.state.contains_creature(fat));
    assert_eq!(c.state.creature(fat).power_amount(PowerKind::Strength), 2);
    let next = |c: &Combat| {
        c.state
            .creature(fat)
            .monster
            .as_ref()
            .unwrap()
            .next_move_name()
    };
    assert_eq!(next(&c), "UNSET_MOVE");
    // Escaped creatures are gone for good.
    with_engine(&mut c, |eng| eng.add_created_enemy(fat));
    with_engine(&mut c, |eng| {
        eng.escape(fat);
        eng.apply_power(fat, PowerKind::Strength, Decimal::from(2), None)
    });
    assert!(!c.state.creature(fat).has_power(PowerKind::Strength));
}

#[test]
fn the_merc_steals_then_leaves_two_gremlins_and_the_gold_when_it_dies() {
    // GremlinMerc.cs, ThieveryPower.cs, SurprisePower.cs, HeistPower.cs, GremlinMercNormal.cs.
    let mut c = facing(Encounter::GremlinMercNormal);
    let merc = enemy(&c);
    let gold = c.state.player.gold;
    for kind in [PowerKind::Surprise, PowerKind::Thievery] {
        assert!(c.state.creature(merc).has_power(kind));
    }
    assert_eq!(c.state.creature(merc).power_amount(PowerKind::Thievery), 20);
    with_engine(&mut c, |eng| {
        thievery::steal_all(eng, merc);
        thievery::steal_all(eng, merc);
        Ok(())
    });
    assert_eq!(c.state.player.gold, gold - 40);
    // The merc dies; the combat goes on with the two gremlins.
    with_engine(&mut c, |eng| {
        eng.kill(&[merc])?;
        eng.check_win_condition().map(drop)
    });
    assert!(c.state.result.is_none());
    let fat = find(&c, |k| matches!(k, MonsterKind::FatGremlin(_))).expect("fat gremlin");
    assert!(find(&c, |k| matches!(k, MonsterKind::SneakyGremlin(_))).is_some());
    assert_eq!(c.state.creature(fat).power_amount(PowerKind::Heist), 40);
    assert_eq!(c.state.creature(fat).slot, Some("fat"));
    assert_eq!(
        c.state.encounter_data,
        EncounterData::GremlinMercNormal {
            gold_was_stolen: true
        }
    );
    // Killing the fat gremlin gives the gold back as a reward.
    with_engine(&mut c, |eng| eng.kill(&[fat]));
    assert_eq!(c.state.extra_rewards, [ExtraReward::Gold(40)]);
}

#[test]
fn a_fleeing_fat_gremlin_keeps_the_gold_and_costs_the_gold_reward() {
    // FatGremlin.cs `FleeMove`, GremlinMercNormal.CalculateGoldProportion: an escape drops the
    // `HeistPower` without a callback; the proportion is 0 with stolen gold, 0.5 without.
    for (steal, expected) in [(true, GoldProportion::ZERO), (false, GoldProportion::HALF)] {
        let mut c = facing(Encounter::GremlinMercNormal);
        let merc = enemy(&c);
        with_engine(&mut c, |eng| {
            if steal {
                thievery::steal_all(eng, merc);
            }
            eng.kill(&[merc])
        });
        let fat = find(&c, |k| matches!(k, MonsterKind::FatGremlin(_))).expect("fat gremlin");
        let sneaky = find(&c, |k| matches!(k, MonsterKind::SneakyGremlin(_))).expect("sneaky");
        with_engine(&mut c, |eng| {
            eng.escape(fat);
            eng.kill(&[sneaky])?;
            eng.check_win_condition().map(drop)
        });
        assert_eq!(c.state.result, Some(CombatResult::Won));
        assert_eq!(c.state.gold_proportion, expected);
        assert_eq!(c.state.extra_rewards, Vec::new());
    }
}

#[test]
fn damage_wakes_the_matriarch_and_strips_its_plating() {
    // LagavulinMatriarch.cs `Sleep`, AsleepPower.AfterDamageReceived.
    let mut c = facing(Encounter::LagavulinMatriarchBoss);
    let matriarch = enemy(&c);
    let p = player(&c);
    assert_eq!(
        c.state.creature(matriarch).power_amount(PowerKind::Asleep),
        3
    );
    assert_eq!(
        c.state.creature(matriarch).power_amount(PowerKind::Plating),
        12
    );
    with_engine(&mut c, |eng| {
        eng.damage(
            &[matriarch],
            Decimal::from(50),
            ValueProps::UNPOWERED,
            Some(p),
            None,
        )
    });
    let m = c.state.creature(matriarch);
    assert!(!m.has_power(PowerKind::Asleep) && !m.has_power(PowerKind::Plating));
    assert_eq!(
        m.monster.as_ref().map(Monster::next_move_name),
        Some("STUNNED")
    );
}

#[test]
fn the_matriarch_sleeps_through_three_turns_then_wakes_into_slash() {
    // AsleepPower: `BeforeSideTurnEndVeryEarly` drops the Plating on the last turn,
    // `AfterSideTurnEnd` counts down; the `SLEEP_BRANCH` reads the power.
    let mut c = facing(Encounter::LagavulinMatriarchBoss);
    let matriarch = enemy(&c);
    let next = |c: &Combat| {
        c.state
            .creature(matriarch)
            .monster
            .as_ref()
            .map(Monster::next_move_name)
    };
    assert_eq!(next(&c), Some("SLEEP_MOVE"));
    for _ in 0..3 {
        assert!(c.state.creature(matriarch).has_power(PowerKind::Asleep));
        c.apply(Action::EndTurn).expect("turn ends");
    }
    assert!(!c.state.creature(matriarch).has_power(PowerKind::Asleep));
    assert!(!c.state.creature(matriarch).has_power(PowerKind::Plating));
    assert_eq!(next(&c), Some("SLASH_MOVE"));
}

#[test]
fn rats_start_on_consecutive_moves_and_call_for_backup_into_the_last_free_slot() {
    // TwoTailedRatsNormal.cs, TwoTailedRat.CallForBackup.
    let mut c = facing(Encounter::TwoTailedRatsNormal);
    let rats = c.state.enemies.clone();
    assert_eq!(rats.len(), 3);
    let starters: Vec<i32> = rats
        .iter()
        .map(|&r| match kind_of(&c, r) {
            MonsterKind::TwoTailedRat(rat) => rat.starter_move_index,
            _ => unreachable!("rats"),
        })
        .collect();
    assert_eq!(starters[1], (starters[0] + 1) % 3);
    assert_eq!(starters[2], (starters[0] + 2) % 3);
    let caller = rats[0];
    let state = 4;
    with_engine(&mut c, |eng| {
        TwoTailedRat::default().perform_move(eng, caller, state)
    });
    assert_eq!(c.state.enemies.len(), 4);
    let slots: Vec<_> = c
        .state
        .enemies
        .iter()
        .map(|&e| c.state.creature(e).slot)
        .collect();
    assert_eq!(
        slots,
        [Some("second"), Some("third"), Some("fourth"), Some("fifth")]
    );
    // Every rat now counts one call.
    for &r in &c.state.enemies {
        assert!(
            matches!(kind_of(&c, r), MonsterKind::TwoTailedRat(rat) if rat.call_for_backup_count == 1)
        );
    }
}
