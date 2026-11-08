//! Hive batch HM4 behaviour the referee cannot check yet (paths under `Models.`): the crab arms'
//! deaths (sts2-cli throws in `Crusher`/`Rocket.BeforeDeath`, whose `NAudioManager.Instance` is null
//! there) and `TheInsatiable`'s Sandpit (the referee stalls on `TheInsatiableBoss` until XT2's IL
//! fix).

use super::*;
use crate::combat::hooks::DamageQuery;
use crate::content::powers::surrounded::Direction;
use crate::content::powers::{PowerData, sandpit};

fn kaiser_crab() -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(Encounter::KaiserCrabBoss))
}

/// The `KaiserCrabBoss` arms: (`Crusher`, `Rocket`).
fn arms(c: &Combat) -> (CreatureId, CreatureId) {
    (c.state.enemies[0], c.state.enemies[1])
}

fn facing(c: &Combat) -> Direction {
    let power = c
        .state
        .creature(player(c))
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Surrounded)
        .expect("Rocket surrounds the player");
    match *c.state.power_data(power.id) {
        PowerData::Surrounded(d) => d.facing,
        _ => unreachable!("Surrounded holds its own data"),
    }
}

/// 10 powered damage from `dealer` to the player, after the damage hooks.
fn hit_player(c: &Combat, dealer: CreatureId) -> Decimal {
    let query = DamageQuery {
        target: Some(player(c)),
        dealer: Some(dealer),
        props: ValueProps::MOVE,
        card: None,
    };
    c.state.hooked().modify_damage(query, Decimal::from(10)).0
}

#[test]
fn surrounded_takes_one_and_a_half_from_behind_and_turns_to_its_target() {
    // Rocket.AfterAddedToRoom puts SurroundedPower on every opponent (Osty is summoned later, at
    // combat start, so not on it); facing Right, the Crusher (BackAttackLeftPower) is behind
    // (SurroundedPower.ModifyDamageMultiplicative).
    let mut c = kaiser_crab();
    let (crusher, rocket) = arms(&c);
    assert!(!c.state.creature(osty(&c)).has_power(PowerKind::Surrounded));
    assert_eq!(facing(&c), Direction::Right);
    assert_eq!(hit_player(&c, crusher), Decimal::from(15));
    assert_eq!(hit_player(&c, rocket), Decimal::from(10));
    // BeforeCardPlayed: a card of its player targeting the arm behind turns it around.
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.apply(Action::PlayCard {
        card: strike,
        target: Some(crusher),
    })
    .expect("legal");
    assert_eq!(facing(&c), Direction::Left);
    assert_eq!(hit_player(&c, crusher), Decimal::from(10));
    assert_eq!(hit_player(&c, rocket), Decimal::from(15));
}

#[test]
fn crab_rage_fires_when_the_other_arm_dies() {
    // CrabRagePower.AfterDeath: another creature of its side died: 6 Strength, 99 unpowered
    // block, then the power removes itself. SurroundedPower.AfterDeath: the hittable enemies
    // left all hold BackAttackLeftPower, so the player turns to face the Crusher.
    let mut c = kaiser_crab();
    let (crusher, rocket) = arms(&c);
    with_engine(&mut c, |eng| eng.kill(&[rocket]));
    let crusher = c.state.creature(crusher);
    assert_eq!(crusher.power_amount(PowerKind::Strength), 6);
    assert_eq!(crusher.block, 99);
    assert!(!crusher.has_power(PowerKind::CrabRage));
    assert!(crusher.has_power(PowerKind::BackAttackLeft));
    assert_eq!(facing(&c), Direction::Left);
}

fn the_insatiable() -> Combat {
    Combat::new(
        CombatConfig::necrobinder(SEED, 0)
            .with_encounter(Encounter::TheInsatiableBoss)
            .with_potions(&[PotionKind::FairyInABottle]),
    )
}

fn sandpit_amount(c: &Combat) -> i32 {
    c.state.creature(enemy(c)).power_amount(PowerKind::Sandpit)
}

#[test]
fn sandpit_at_zero_kills_the_player_through_death_prevention() {
    // SandpitPower.AfterSideTurnStartLate decrements it on the enemy side's turn start; at 0
    // PowerCmd.Remove runs AfterRemoved, which kills the target's player and Osty with
    // `force: true`, so Fairy in a Bottle does not save them (CreatureCmd.cs:537).
    let mut c = the_insatiable();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| sandpit::apply(eng, e, p, Decimal::from(2)));
    c.apply(Action::EndTurn).expect("legal");
    // Liquify Ground added a second Sandpit of 4 on the first enemy turn.
    let amounts: Vec<i32> = c
        .state
        .creature(e)
        .powers
        .iter()
        .filter(|p| p.kind == PowerKind::Sandpit)
        .map(|p| p.amount)
        .collect();
    assert_eq!(amounts, [1, 4]);
    assert_eq!(c.status(), Status::PlayerTurn);
    c.apply(Action::EndTurn).expect("legal");
    assert_eq!(c.status(), Status::Lost);
    assert!(c.state.creature(p).is_dead());
    assert!(c.state.creature(osty(&c)).is_dead());
    assert_eq!(belt(&c)[0], Some(PotionKind::FairyInABottle), "never used");
}

#[test]
fn sandpit_spares_the_player_once_its_owner_is_dead() {
    // SandpitPower.AfterRemoved returns when the old owner (or the target) is dead: the
    // owner's death removes it with the rest of its powers.
    let mut c = the_insatiable();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| sandpit::apply(eng, e, p, Decimal::ONE));
    with_engine(&mut c, |eng| eng.kill(&[e]));
    assert!(c.state.creature(p).is_alive());
}

#[test]
fn frantic_escape_raises_the_sandpit_and_costs_more() {
    // FranticEscape.OnPlay: ModifyAmount(+1) on the first enemy's Sandpit targeting its owner,
    // then EnergyCost.AddThisCombat(1).
    let mut c = the_insatiable();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| sandpit::apply(eng, e, p, Decimal::from(4)));
    let card = give_card(&mut c, CardKind::FranticEscape, 0);
    c.apply(Action::PlayCard { card, target: None })
        .expect("legal");
    assert_eq!(sandpit_amount(&c), 5);
    assert_eq!(c.state.energy_cost(card), 2);
}
