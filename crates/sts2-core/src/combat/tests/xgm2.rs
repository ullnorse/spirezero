//! Glory batch GM2 powers that `KnightsElite` uses, until it can be diffed: `HexPower` and
//! `DampenPower` (paths under `Models.Powers/`).

use super::*;
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::CardKeyword;

/// Three Scrolls of Biting, so one dying does not end the combat.
fn three_enemies() -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(Encounter::ScrollsOfBitingWeak))
}

#[test]
fn hex_makes_every_card_ethereal_until_its_applier_dies() {
    // HexPower.cs: AfterApplied afflicts every card without an affliction with Hexed, which
    // TryModifyKeywordsInCombat makes Ethereal; the applier's death removes the power and every
    // Hexed affliction.
    let mut c = three_enemies();
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Hex, Decimal::from(2), Some(e))
    });
    let hexed = c.state.card(strike).affliction.expect("afflicted");
    assert_eq!((hexed.kind, hexed.amount), (AfflictionKind::Hexed, 2));
    assert!(c.state.has_keyword(strike, CardKeyword::Ethereal));
    // A card entering combat gets Hexed too (AfterCardEnteredCombat).
    let wound = with_engine(&mut c, |eng| {
        let card = eng.create_card(CardKind::Dazed);
        eng.add_generated_card_to_combat(card, PileType::Discard, None)?;
        Ok(card)
    });
    assert!(c.state.has_keyword(wound, CardKeyword::Ethereal));
    with_engine(&mut c, |eng| eng.kill(&[e]));
    assert!(!c.state.creature(p).has_power(PowerKind::Hex));
    assert_eq!(c.state.card(strike).affliction, None);
    assert!(!c.state.has_keyword(strike, CardKeyword::Ethereal));
}

#[test]
fn dampen_downgrades_upgraded_cards_until_its_last_caster_dies() {
    // DampenPower.cs: AfterApplied downgrades every upgraded combat card; AfterDeath of the last
    // caster removes the power, and AfterRemoved upgrades the cards back (not once combat is
    // ending: CardCmd.Upgrade).
    let mut c = three_enemies();
    let upgraded = give_card(&mut c, CardKind::StrikeNecrobinder, 1);
    let plain = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        crate::content::powers::dampen::apply_from_caster(eng, p, e)
    });
    assert!(c.state.creature(p).has_power(PowerKind::Dampen));
    assert_eq!(c.state.card(upgraded).upgrade_level, 0);
    assert_eq!(c.state.card(plain).upgrade_level, 0);
    with_engine(&mut c, |eng| eng.kill(&[e]));
    assert!(!c.state.creature(p).has_power(PowerKind::Dampen));
    assert_eq!(c.state.card(upgraded).upgrade_level, 1);
    assert_eq!(c.state.card(plain).upgrade_level, 0);
}
