//! Glory bosses (GM3) behaviour the forced diffs rarely or never reach (paths under `Models.`):
//! `TestSubject`'s dead state must be performed before it rolls again, and `ImprovementPower`
//! (given only by `MadScience`) upgrades deck cards when the combat ends.

use super::*;
use crate::content::monsters;

fn test_subject() -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(Encounter::TestSubjectBoss))
}

fn next_move(c: &Combat, id: CreatureId) -> &'static str {
    c.state
        .creature(id)
        .monster
        .as_ref()
        .expect("a monster")
        .next_move_name()
}

#[test]
fn test_subject_revives_through_its_dead_state() {
    // TestSubject.cs:139-145,233-258, AdaptablePower.cs:33-70,
    // MonsterMoveStateMachine.cs:53-90 (`CanTransitionAway`), MoveState.cs:27-37.
    let mut c = test_subject();
    let me = enemy(&c);
    with_engine(&mut c, |eng| eng.kill(&[me]));
    assert!(c.state.creature(me).is_dead());
    assert!(
        c.state.enemies.contains(&me),
        "Adaptable keeps it in combat"
    );
    assert!(
        c.state.in_progress,
        "Adaptable stops the combat from ending"
    );
    assert!(c.state.creature(me).has_power(PowerKind::Adaptable));
    assert!(!c.state.creature(me).has_power(PowerKind::Enrage));
    assert_eq!(next_move(&c, me), "RESPAWN_MOVE");
    // Not performed yet: a roll keeps the dead state.
    with_engine(&mut c, |eng| {
        monsters::roll_move(eng, me);
        Ok(())
    });
    assert_eq!(next_move(&c, me), "RESPAWN_MOVE");
    with_engine(&mut c, |eng| monsters::perform_move(eng, me));
    let creature = c.state.creature(me);
    assert_eq!((creature.hp, creature.max_hp), (200, 200));
    assert!(creature.has_power(PowerKind::PainfulStabs));
    with_engine(&mut c, |eng| {
        monsters::roll_move(eng, me);
        Ok(())
    });
    assert_eq!(next_move(&c, me), "MULTI_CLAW_MOVE");
}

#[test]
fn improvement_upgrades_distinct_deck_cards_when_the_combat_ends() {
    // ImprovementPower.cs:17-30, CombatManager.cs:1297-1315 (not ending once not in progress).
    let mut c = new_combat();
    let me = player(&c);
    let enemies = c.state.enemies.clone();
    let draws = c.state.rng[RunRngType::CombatCardSelection].counter();
    with_engine(&mut c, |eng| {
        eng.apply_power(me, PowerKind::Improvement, Decimal::from(3), Some(me))?;
        eng.kill(&enemies)?;
        eng.check_win_condition().map(drop)
    });
    assert_eq!(c.state.result, Some(CombatResult::Won));
    let upgraded = c.state.deck.iter().filter(|d| d.upgrade_level == 1).count();
    assert_eq!(upgraded, 3);
    assert_eq!(
        c.state.rng[RunRngType::CombatCardSelection].counter(),
        draws + 3
    );
}
