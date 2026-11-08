//! Review follow-ups (PX26): summons keep the hook unions in step with a fresh count (they
//! kept the old listener cache in step with a fresh build), and Shroud and Sleight of Flesh
//! find a Doom its owner's death removed.

use super::*;
use crate::content::monsters::{EyeWithTeeth, Parafright, Stabbot, Zapbot};

fn facing(encounter: Encounter) -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(encounter))
}

/// Runs `f` on the state, then checks its hook unions against a fresh count.
fn with_fresh_cache<T>(c: &mut Combat, f: impl FnOnce(&mut Engine<'_>) -> Flow<T>) -> T {
    assert!(c.state.hook_unions_are_exact(), "the unions differ before");
    let out = {
        let mut eng = Engine::new(&mut c.state, c.events.as_mut(), &[]);
        f(&mut eng).expect("no decision needed")
    };
    assert!(
        c.state.hook_unions_are_exact(),
        "the unions differ from a fresh count"
    );
    out
}

fn slots(c: &Combat) -> Vec<Option<&'static str>> {
    c.state
        .enemies
        .iter()
        .map(|&e| c.state.creature(e).slot)
        .collect()
}

const EYE: MonsterKind = MonsterKind::EyeWithTeeth(EyeWithTeeth);

#[test]
fn summons_into_a_shared_slot_keep_the_listener_cache() {
    // Fogmog.cs:62-68, TheObscura.cs:78-87: every illusion takes "illusion", the encounter's
    // lowest slot (FogmogNormal.cs:15, TheObscuraNormal.cs:15), so the 3-item sort
    // (CombatState.cs:496-502) never finds a tie above the newcomer.
    let mut c = facing(Encounter::FogmogNormal);
    for _ in 0..3 {
        with_fresh_cache(&mut c, |eng| eng.add_enemy(EYE, Some("illusion")));
    }
    assert_eq!(
        slots(&c),
        [
            Some("illusion"),
            Some("illusion"),
            Some("illusion"),
            Some("fogmog")
        ]
    );
    let mut c = facing(Encounter::TheObscuraNormal);
    for _ in 0..2 {
        with_fresh_cache(&mut c, |eng| {
            eng.add_enemy(MonsterKind::Parafright(Parafright), Some("illusion"))
        });
    }
    assert_eq!(
        slots(&c),
        [Some("illusion"), Some("illusion"), Some("obscura")]
    );
}

#[test]
fn death_summons_keep_the_listener_cache() {
    // StockPower.cs:17-28: the respawn takes its dead owner's slot while the owner is still in
    // `Enemies` (CreatureCmd.cs:551-559 removes it after `AfterDeath`); AxebotsNormal.cs:11 has
    // that one slot.
    let mut c = facing(Encounter::AxebotsNormal);
    let axebot = enemy(&c);
    with_fresh_cache(&mut c, |eng| eng.kill(&[axebot]));
    assert_eq!(slots(&c), [Some("front")]);
    assert_ne!(c.state.enemies, [axebot]);
    // InfestedPower.cs:22-33: four wrigglers in distinct slots after the dying phrog's.
    let mut c = facing(Encounter::PhrogParasiteElite);
    let phrog = enemy(&c);
    with_fresh_cache(&mut c, |eng| eng.kill(&[phrog]));
    assert_eq!(
        slots(&c),
        [
            Some("wriggler1"),
            Some("wriggler2"),
            Some("wriggler3"),
            Some("wriggler4")
        ]
    );
}

#[test]
fn next_slot_summons_keep_the_listener_cache() {
    // Fabricator.cs:115: `GetNextSlot` (EncounterModel.cs:240-243) never hands out a taken slot;
    // FabricatorNormal.cs:19 puts the Fabricator between the bots.
    let mut c = facing(Encounter::FabricatorNormal);
    for bot in [
        MonsterKind::Zapbot(Zapbot),
        MonsterKind::Stabbot(Stabbot),
        MonsterKind::Zapbot(Zapbot),
    ] {
        with_fresh_cache(&mut c, |eng| {
            let slot = eng.s.encounter.next_slot(eng.s);
            eng.add_enemy(bot, slot)
        });
    }
    assert_eq!(
        slots(&c),
        [Some("bot1"), Some("bot2"), Some("fabricator"), Some("bot3")]
    );
}

#[test]
fn the_three_item_sort_swaps_a_tie_above_the_newcomer() {
    // Why the cache needs the precondition above: .NET's `SwapIfGreater` sort of three items is
    // not stable, so two enemies tied above a lower newcomer change places. No encounter puts
    // two enemies in a slot above another one's.
    let mut c = facing(Encounter::FogmogNormal);
    let fogmog = enemy(&c);
    let tied = create_enemy(&mut c.state, EYE, Some("fogmog"));
    c.state.push_enemy(tied);
    c.state.sort_enemies_by_slot_name();
    assert_eq!(c.state.enemies, [fogmog, tied]);
    let lower = create_enemy(&mut c.state, EYE, Some("illusion"));
    c.state.push_enemy(lower);
    c.state.sort_enemies_by_slot_name();
    assert_eq!(c.state.enemies, [lower, tied, fogmog]);
}

#[test]
fn shroud_and_sleight_of_flesh_read_a_doom_removed_by_death() {
    // ShroudPower.cs, SleightOfFleshPower.cs: they test the `PowerModel` they are handed, which
    // keeps its type once its owner's death removes it (Creature.cs:673). Sleight of Flesh's
    // damage kills the illusion before Shroud runs; Shroud still sees Doom. The Fogmog keeps the
    // combat going, or the pass would stop at the death.
    let mut c = facing(Encounter::FogmogNormal);
    let p = player(&c);
    let e = with_fresh_cache(&mut c, |eng| {
        let eye = eng.add_enemy(EYE, Some("illusion"))?;
        eng.apply_power(p, PowerKind::SleightOfFlesh, Decimal::from(5), Some(p))?;
        eng.apply_power(p, PowerKind::Shroud, Decimal::from(3), Some(p))?;
        Ok(eye)
    });
    c.state.creature_mut(e).hp = 1;
    let block = c.state.creature(p).block;
    with_engine(&mut c, |eng| {
        eng.apply_power(e, PowerKind::Doom, Decimal::from(2), Some(p))
    });
    let enemy_c = c.state.creature(e);
    assert!(enemy_c.is_dead());
    assert!(
        enemy_c
            .removed_powers
            .iter()
            .any(|p| p.kind == PowerKind::Doom)
    );
    assert_eq!(c.state.creature(p).block, block + 3);
}
