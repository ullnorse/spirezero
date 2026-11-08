//! `DamageReceived` history entries, the Fatal helpers and `ModifyDamage` for another card
//! (C18; paths under `ref/decomp/MegaCrit.Sts2.Core.`).

use super::*;
use crate::combat::commands::damage::DamageResults;
use crate::combat::history::{HistoryKind, HistoryTag};
use crate::combat::hooks::DamageQuery;

fn give_power(c: &mut Combat, creature: CreatureId, kind: PowerKind, amount: i32) {
    let applier = Some(player(c));
    with_engine(c, |eng| {
        eng.apply_power(creature, kind, Decimal::from(amount), applier)
    });
}

/// A combat whose Osty is dead, so hits on the player are not redirected.
fn combat_without_osty() -> Combat {
    let mut c = new_combat();
    let osty = osty(&c);
    with_engine(&mut c, |eng| eng.kill(&[osty]));
    c
}

/// The enemy hits the player for `amount` (no card source).
fn enemy_hits_player(c: &mut Combat, amount: i32) -> DamageResults {
    let (p, e) = (player(c), enemy(c));
    with_engine(c, |eng| {
        eng.damage(&[p], Decimal::from(amount), ValueProps::MOVE, Some(e), None)
    })
}

#[test]
fn damage_received_is_recorded_per_result_even_when_fully_blocked() {
    // CreatureCmd.cs:316-320: one entry per DamageResult, receiver = actor, UnblockedDamage 0
    // for a fully blocked hit; Spite counts only entries with UnblockedDamage > 0.
    let mut c = combat_without_osty();
    let p = player(&c);
    c.state.creature_mut(p).block = 10;
    enemy_hits_player(&mut c, 4);
    let entries: Vec<_> = c
        .state
        .history_this_turn(HistoryTag::DamageReceived)
        .iter()
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].actor, p);
    assert!(matches!(
        entries[0].kind,
        HistoryKind::DamageReceived { result, dealer: Some(d), card_source: None, .. }
            if result.unblocked == 0 && result.blocked == 4 && d == enemy(&c)
    ));
    assert!(!c.state.lost_hp_this_turn(p));
    assert_eq!(c.state.hp_loss_hits_this_combat(p), 0);

    // A hit that gets through is "Spite's" condition; a hit on the enemy is not the player's.
    enemy_hits_player(&mut c, 20);
    assert!(c.state.lost_hp_this_turn(p));
    assert!(!c.state.lost_hp_this_turn(enemy(&c)));
    assert_eq!(c.state.history_count(HistoryTag::DamageReceived), 2);
}

#[test]
fn spite_and_tear_asunder_queries_over_turns() {
    // Spite.cs LostHpThisTurn is per turn; TearAsunder.cs counts the whole combat (hits = 1 +
    // count of entries with receiver == owner and UnblockedDamage > 0).
    let mut c = combat_without_osty();
    let p = player(&c);
    for _ in 0..3 {
        enemy_hits_player(&mut c, 1);
    }
    assert!(c.state.lost_hp_this_turn(p));
    assert_eq!(1 + c.state.hp_loss_hits_this_combat(p), 4);

    // The next turn: nothing this turn, the combat count stays.
    c.state.round += 1;
    assert!(!c.state.lost_hp_this_turn(p));
    assert!(
        c.state
            .history_this_turn(HistoryTag::DamageReceived)
            .is_empty()
    );
    enemy_hits_player(&mut c, 1);
    assert!(c.state.lost_hp_this_turn(p));
    assert_eq!(1 + c.state.hp_loss_hits_this_combat(p), 5);
}

#[test]
fn damage_received_keeps_the_card_source_and_props() {
    // DamageReceivedEntry.cs: Result (with Props), Dealer, CardSource, Receiver = Actor.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    with_engine(&mut c, |eng| {
        eng.damage(
            &[e],
            Decimal::from(6),
            ValueProps::MOVE,
            Some(p),
            Some(strike),
        )
    });
    let entries: Vec<_> = c
        .state
        .history_this_turn(HistoryTag::DamageReceived)
        .iter()
        .collect();
    assert!(matches!(
        entries[0].kind,
        HistoryKind::DamageReceived { dealer: Some(d), card_source: Some(card), props, .. }
            if d == p && card == strike && props == ValueProps::MOVE
    ));
    assert_eq!(entries[0].actor, e);
}

#[test]
fn fatal_reads_the_target_powers_before_the_attack() {
    // Feed.cs / HandOfGreed.cs / TheHunt.cs: every power must allow Fatal (MinionPower.cs vetoes
    // it) and some result must have WasTargetKilled.
    let mut c = new_combat();
    let e = enemy(&c);
    with_engine(&mut c, |eng| {
        assert!(eng.target_allows_fatal(e));
        Ok(())
    });
    give_power(&mut c, e, PowerKind::Minion, 1);
    with_engine(&mut c, |eng| {
        assert!(!eng.target_allows_fatal(e));
        Ok(())
    });

    let hp = c.state.creature(e).hp;
    let p = player(&c);
    let kill = |c: &mut Combat, amount: i32| {
        with_engine(c, |eng| {
            eng.damage(&[e], Decimal::from(amount), ValueProps::MOVE, Some(p), None)
        })
    };
    let results = kill(&mut c, hp - 1);
    assert!(!Engine::fatal_triggered(&results));
    let results = kill(&mut c, 1);
    assert!(Engine::fatal_triggered(&results));
    assert!(c.state.creature(e).is_dead());
}

#[test]
fn modify_damage_accepts_no_target_and_another_card() {
    // Thrash.cs:70: Hook.ModifyDamage(target null, dealer owner, damage, Move, otherCard,
    // cardPlay null, All): the player's Strength still applies, no target-side power does.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    let other = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    give_power(&mut c, p, PowerKind::Strength, 2);
    give_power(&mut c, e, PowerKind::Vulnerable, 2);
    let query = DamageQuery {
        target: None,
        dealer: Some(p),
        props: ValueProps::MOVE,
        card: Some(other),
    };
    let (damage, _) = with_engine(&mut c, |eng| {
        Ok(eng.hooked().modify_damage(query, Decimal::from(5)))
    });
    assert_eq!(damage, Decimal::from(7));
}
