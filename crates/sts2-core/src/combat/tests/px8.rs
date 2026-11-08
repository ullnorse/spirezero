//! Listeners whose power, or whose owner, leaves mid-hook (PX8): a C# `PowerModel` outlives its
//! removal (`PowerModel.RemoveInternal`, `PowerModel.cs:583`), and the hook snapshot re-checks
//! each listener before calling it (`CombatState.cs:411-486`).

use super::*;
use crate::combat::hooks::{Listener, PowerRef};
use crate::content::powers::poison;

fn facing(encounter: Encounter) -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(encounter))
}

fn power_of(c: &Combat, owner: CreatureId, kind: PowerKind) -> PowerId {
    c.state
        .creature(owner)
        .powers
        .iter()
        .find(|p| p.kind == kind)
        .map(|p| p.id)
        .expect("applied")
}

#[test]
fn poison_keeps_triggering_on_a_waterfall_giant_whose_death_stripped_it() {
    // PoisonPower.cs:66-81: with Accelerant the loop runs twice. The first hit kills the
    // giant; SteamEruption keeps it in combat and resets it to 999999999 HP
    // (SteamEruptionPower.cs:15-21, WaterfallGiant.cs:300-307) while the death removes the
    // Poison (Creature.cs:673). The removed instance still hits for its own Amount and
    // decrements, every listener hook included (PowerCmd.cs:219-265 checks only the owner's
    // CombatState). Referee-checked as sts2-diff's run_fixtures/waterfall_giant_poison_revive.json.
    let mut c = facing(Encounter::WaterfallGiantBoss);
    let (p, giant) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(
            giant,
            PowerKind::SteamEruption,
            Decimal::from(10),
            Some(giant),
        )?;
        eng.apply_power(giant, PowerKind::Poison, Decimal::from(5), Some(p))?;
        eng.apply_power(p, PowerKind::Accelerant, Decimal::ONE, Some(p))
    });
    c.state.creature_mut(giant).hp = 2;
    let me = PowerRef {
        owner: giant,
        id: power_of(&c, giant, PowerKind::Poison),
    };
    with_engine(&mut c, |eng| poison::trigger(eng, me));
    let giant_c = c.state.creature(giant);
    assert!(!giant_c.has_power(PowerKind::Poison));
    assert!(giant_c.has_power(PowerKind::SteamEruption));
    // 5 killed it, then the detached Poison (now 4) hit the reset giant.
    assert_eq!(giant_c.hp, 999_999_999 - 4);
    assert_eq!(c.state.power(giant, me.id).amount, 3);
    assert!(c.state.enemies.contains(&giant));
}

#[test]
fn a_listener_whose_owner_died_earlier_in_the_hook_is_skipped() {
    // CombatState.cs:411-486: the snapshot entry of a power that left its owner (here with
    // the owner's death, CreatureCmd.cs:563) is not called. DemonForm listens after Poison.
    let mut c = facing(Encounter::SlimesWeak);
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(e, PowerKind::Poison, Decimal::from(5), Some(p))?;
        eng.apply_power(e, PowerKind::DemonForm, Decimal::from(2), Some(e))
    });
    c.state.creature_mut(e).hp = 3;
    let enemies = c.state.enemies.clone();
    with_engine(&mut c, |eng| {
        eng.after_side_turn_start(Side::Enemy, &enemies)
    });
    let e_c = c.state.creature(e);
    assert!(e_c.is_dead());
    assert!(!e_c.has_power(PowerKind::Strength));
    assert_eq!(e_c.powers, []);
}

#[test]
fn an_escaped_creatures_power_keeps_its_fields_and_ignores_modify_amount() {
    // CreatureCmd.cs:609-633: Escape removes every power without callbacks and the creature
    // leaves the combat (`CombatState = null`, CombatState.cs:278-303), so a later
    // `ModifyAmount` on one of its instances does nothing (PowerCmd.cs:226-230).
    let mut c = facing(Encounter::SlimesWeak);
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(e, PowerKind::Poison, Decimal::from(5), Some(p))
    });
    let poison = power_of(&c, e, PowerKind::Poison);
    with_engine(&mut c, |eng| {
        eng.escape(e);
        eng.decrement_power(PowerRef {
            owner: e,
            id: poison,
        })
    });
    assert_eq!(c.state.creature(e).powers, []);
    assert!(!c.state.contains_listener(Listener::Power {
        owner: e,
        power: poison
    }));
    assert_eq!(c.state.power(e, poison).amount, 5);
}

#[test]
fn removing_a_removed_power_runs_its_after_removed_again() {
    // PowerCmd.cs:286-295: `RemoveInternal` on a detached instance removes nothing, but
    // `AfterRemoved` still runs. Tangled's clears the Entangled afflictions again.
    let mut c = new_combat();
    let p = player(&c);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Tangled, Decimal::ONE, None)
    });
    let tangled = power_of(&c, p, PowerKind::Tangled);
    with_engine(&mut c, |eng| {
        eng.remove_power(p, tangled)?;
        eng.remove_power(p, tangled)
    });
    assert_eq!(c.state.card(strike).affliction, None);
    assert!(!c.state.creature(p).has_power(PowerKind::Tangled));
    assert_eq!(c.state.creature(p).removed_powers.len(), 1);
}
