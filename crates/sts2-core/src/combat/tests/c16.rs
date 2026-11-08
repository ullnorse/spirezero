//! Orbs: channel, evoke, passives, slots, Focus and history (C16; paths under
//! `ref/decomp/MegaCrit.Sts2.Core.`, spec `docs/specs/orbs.md`).

use super::*;

/// A Defect combat on turn 1, after `CrackedCore` channeled its Lightning orb.
fn defect_combat() -> Combat {
    Combat::new(
        CombatConfig::new(Character::Defect, SEED, 0)
            .expect("Defect's starter is ported")
            .with_events(),
    )
}

fn set_focus(c: &mut Combat, amount: i32) {
    let p = player(c);
    with_engine(c, |eng| {
        eng.apply_power(p, PowerKind::Focus, Decimal::from(amount), Some(p))
    });
}

fn kinds(c: &Combat) -> Vec<OrbKind> {
    c.state.player.orbs.iter().map(|o| o.kind).collect()
}

fn values(c: &Combat, orb: usize) -> (Decimal, Decimal) {
    let orb = c.state.player.orbs[orb];
    (c.state.orb_passive_val(orb), c.state.orb_evoke_val(orb))
}

#[test]
fn defect_starts_with_three_slots_and_cracked_cores_lightning() {
    // Defect.cs:63 (BaseOrbSlotCount 3), CrackedCore.cs:29-35, OrbChanneledEntry.
    let c = defect_combat();
    assert_eq!(c.state.player.orb_slots, 3);
    assert_eq!(kinds(&c), [OrbKind::Lightning]);
    assert_eq!(c.state.orbs_channeled_this_combat(OrbKind::Lightning), 1);
}

#[test]
fn focus_modifies_orb_values_but_not_darks_starting_evoke() {
    // FocusPower.cs:14 (max(value + Amount, 0)); DarkOrb.cs:17,23-40: _evokeVal starts at a flat
    // 6 and grows by the Focus-modified passive.
    let mut c = defect_combat();
    with_engine(&mut c, |eng| eng.channel(OrbKind::Dark));
    set_focus(&mut c, 2);
    let n = |v: i32| Decimal::from(v);
    assert_eq!(values(&c, 0), (n(5), n(10)));
    assert_eq!(values(&c, 1), (n(8), n(6)));
    let dark = c.state.player.orbs[1];
    with_engine(&mut c, |eng| eng.orb_passive(dark, None, false));
    assert_eq!(values(&c, 1), (n(8), n(14)));
    set_focus(&mut c, -12);
    assert_eq!(values(&c, 0), (n(0), n(0)));
    assert_eq!(values(&c, 1), (n(0), n(14)));
}

#[test]
fn glass_decays_by_one_per_passive_and_evokes_double() {
    // GlassOrb.cs:15-38: PassiveVal = Mod(_passiveVal), decremented before its damage;
    // EvokeVal = PassiveVal * 2.
    let mut c = defect_combat();
    with_engine(&mut c, |eng| eng.channel(OrbKind::Glass));
    let n = |v: i32| Decimal::from(v);
    assert_eq!(values(&c, 1), (n(4), n(8)));
    let e = enemy(&c);
    let hp = c.state.creature(e).hp;
    let glass = c.state.player.orbs[1];
    with_engine(&mut c, |eng| eng.orb_passive(glass, None, false));
    assert_eq!(c.state.creature(e).hp, hp - 4);
    assert_eq!(values(&c, 1), (n(3), n(6)));
}

#[test]
fn channeling_into_a_full_queue_evokes_the_front_orb() {
    // OrbCmd.cs:68-92: Orbs.Count >= Capacity evokes the front orb (dequeued) first.
    let mut c = defect_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.channel(OrbKind::Frost)?;
        eng.channel(OrbKind::Frost)?;
        eng.channel(OrbKind::Plasma)
    });
    assert_eq!(kinds(&c), [OrbKind::Frost, OrbKind::Frost, OrbKind::Plasma]);
    let block = c.state.creature(p).block;
    with_engine(&mut c, |eng| eng.channel(OrbKind::Dark));
    assert_eq!(kinds(&c), [OrbKind::Frost, OrbKind::Plasma, OrbKind::Dark]);
    assert_eq!(c.state.creature(p).block, block + 5);
}

#[test]
fn lightning_draws_a_target_even_with_one_enemy() {
    // LightningOrb.cs:43-64, Rng.cs:289-299: NextItem over a list of 1 still draws.
    let mut c = defect_combat();
    for &e in &c.state.enemies.clone()[1..] {
        c.state.creature_mut(e).hp = 0;
    }
    let before = c.state.rng[RunRngType::CombatTargets].counter();
    with_engine(&mut c, |eng| eng.evoke_next(true));
    assert_eq!(c.state.rng[RunRngType::CombatTargets].counter(), before + 1);
    assert_eq!(kinds(&c), []);
}

#[test]
fn a_slotless_character_gets_one_slot_on_its_first_channel() {
    // OrbCmd.cs:76-79: BaseOrbSlotCount == 0 && Capacity == 0 adds one slot.
    let mut c = new_combat();
    assert_eq!(c.state.player.orb_slots, 0);
    with_engine(&mut c, |eng| eng.channel(OrbKind::Frost));
    assert_eq!(c.state.player.orb_slots, 1);
    assert_eq!(kinds(&c), [OrbKind::Frost]);
}

#[test]
fn slots_are_capped_at_ten_and_removing_drops_the_last_orbs() {
    // OrbCmd.cs:23-49, OrbQueue.cs:41-48.
    let mut c = defect_combat();
    with_engine(&mut c, |eng| {
        eng.channel(OrbKind::Frost)?;
        eng.channel(OrbKind::Dark)?;
        eng.add_orb_slots(20);
        Ok(())
    });
    assert_eq!(c.state.player.orb_slots, 10);
    with_engine(&mut c, |eng| {
        eng.remove_orb_slots(9);
        Ok(())
    });
    assert_eq!(c.state.player.orb_slots, 1);
    assert_eq!(kinds(&c), [OrbKind::Lightning]);
}

#[test]
fn a_dead_player_loses_orbs_and_slots() {
    // CreatureCmd.cs:578: OrbQueue.Clear.
    let mut c = defect_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| eng.kill(&[p]));
    assert_eq!(c.state.player.orb_slots, 0);
    assert_eq!(kinds(&c), []);
}
