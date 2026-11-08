//! Underdocks engine mechanics (C22): `CreatureCmd.Escape`, `CalculateGoldProportion` and
//! summoning a created creature.

use super::*;
use crate::content::monsters::EyeWithTeeth;
use crate::run::odds::GoldProportion;

fn facing(encounter: Encounter) -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(encounter))
}

const EYE: MonsterKind = MonsterKind::EyeWithTeeth(EyeWithTeeth);

#[test]
fn an_escaped_enemy_leaves_alive_and_without_powers() {
    // CreatureCmd.cs:609-632, CombatState.cs:267-299: RemoveAllPowersInternalExcept, then the
    // creature leaves `Enemies` and joins `EscapedCreatures`; it is not killed.
    let mut c = facing(Encounter::FogmogNormal);
    let fogmog = enemy(&c);
    let eye = with_engine(&mut c, |eng| eng.add_enemy(EYE, Some("illusion")));
    // Illusion and Minion.
    assert_eq!(c.state.creature(eye).powers.len(), 2);
    with_engine(&mut c, |eng| {
        eng.escape(eye);
        Ok(())
    });
    let eye_c = c.state.creature(eye);
    assert!(eye_c.is_alive() && eye_c.powers.is_empty());
    assert_eq!(c.state.enemies, [fogmog]);
    assert_eq!(c.state.escaped, [eye]);
    assert!(!c.state.contains_creature(eye) && !c.state.is_ending());
    // Escaping twice, or a dead creature, does nothing more.
    with_engine(&mut c, |eng| {
        eng.escape(eye);
        Ok(())
    });
    assert_eq!(c.state.escaped, [eye]);
    with_engine(&mut c, |eng| eng.kill(&[fogmog]));
    with_engine(&mut c, |eng| {
        eng.escape(fogmog);
        Ok(())
    });
    assert_eq!(c.state.escaped, [eye]);
}

#[test]
fn the_last_enemy_escaping_ends_the_fight_with_no_gold() {
    // EncounterModel.cs:368-371: `1f - escaped / SpawnedEnemies.Count`, computed in
    // `CombatRoom.OnCombatEnded` (CombatManager.cs:1316-1317).
    let mut c = facing(Encounter::FogmogNormal);
    let fogmog = enemy(&c);
    with_engine(&mut c, |eng| {
        eng.escape(fogmog);
        eng.check_win_condition().map(drop)
    });
    assert_eq!(c.state.result, Some(CombatResult::Won));
    assert_eq!(c.state.gold_proportion, GoldProportion::ZERO);
}

#[test]
fn gold_proportion_counts_distinct_spawned_models() {
    // EncounterModel.cs:397-414 `OnCreatureSpawned`: a model spawned twice counts once, summons
    // included; escapes count per creature.
    let mut c = facing(Encounter::FogmogNormal);
    let fogmog = enemy(&c);
    let first = with_engine(&mut c, |eng| eng.add_enemy(EYE, Some("illusion")));
    with_engine(&mut c, |eng| eng.kill(&[first]));
    let second = with_engine(&mut c, |eng| eng.add_enemy(EYE, None));
    assert_eq!(c.state.spawned_enemies, ["FOGMOG", "EYE_WITH_TEETH"]);
    with_engine(&mut c, |eng| {
        eng.escape(second);
        eng.kill(&[fogmog])?;
        eng.check_win_condition().map(drop)
    });
    assert_eq!(c.state.result, Some(CombatResult::Won));
    assert_eq!(c.state.gold_proportion, GoldProportion::HALF);
}

#[test]
fn a_won_fight_without_escapes_keeps_all_gold() {
    let mut c = facing(Encounter::SlimesWeak);
    assert_eq!(c.state.spawned_enemies.len(), 3);
    let enemies = c.state.enemies.clone();
    with_engine(&mut c, |eng| {
        eng.kill(&enemies)?;
        eng.check_win_condition().map(drop)
    });
    assert_eq!(c.state.gold_proportion, GoldProportion::ONE);
}

#[test]
fn a_created_enemy_joins_only_when_added() {
    // CombatState.cs:233-248 then CreatureCmd.cs:53-82 (`SurprisePower` creates the fat gremlin,
    // powers it, and adds it after another summon): the HP roll and the `CombatId` happen at
    // creation, the slot sort and `AfterAddedToRoom` at the add.
    let mut c = facing(Encounter::FogmogNormal);
    let fogmog = enemy(&c);
    let niche = c.state.rng[RunRngType::Niche].counter();
    let eye = with_engine(&mut c, |eng| Ok(eng.create_enemy(EYE, Some("illusion"))));
    assert_eq!(c.state.rng[RunRngType::Niche].counter(), niche + 1);
    assert!(!c.state.contains_creature(eye));
    assert_eq!(c.state.spawned_enemies, ["FOGMOG", "EYE_WITH_TEETH"]);
    assert_eq!(c.state.encounter.next_slot(&c.state), Some("illusion"));
    with_engine(&mut c, |eng| eng.add_created_enemy(eye));
    assert_eq!(c.state.enemies, [eye, fogmog]);
    assert!(c.state.creature(eye).has_power(PowerKind::Illusion));
    // `GetNextSlot`: every slot is held now.
    assert_eq!(c.state.encounter.next_slot(&c.state), None);
}
