//! Defect orb powers (PD3; paths under `ref/decomp/MegaCrit.Sts2.Core.`, spec
//! `docs/specs/defect.md` batch 3).

use super::*;
use crate::content::cards::CardKind;

#[test]
fn trash_to_treasure_channels_a_random_orb_per_status_the_owner_generates() {
    // TrashToTreasurePower.cs:16-24: only Status cards created by the owner. The only shipped
    // generators (BoostAway, GunkUp) arrive with Defect batch 4, so no trace reaches this.
    let mut c = Combat::new(
        CombatConfig::new(Character::Defect, SEED, 0)
            .expect("Defect's starter is ported")
            .with_events(),
    );
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::TrashToTreasure, Decimal::from(2), Some(p))
    });
    let before = c.state.player.orbs.len();
    let mut mirror = c.state.rng[RunRngType::CombatOrbs].clone();
    let expected = [
        *mirror.next_item(&OrbKind::ALL).expect("five kinds"),
        *mirror.next_item(&OrbKind::ALL).expect("five kinds"),
    ];
    // A Status from another source (a monster) does nothing.
    with_engine(&mut c, |eng| {
        let dazed = eng.create_card(CardKind::Dazed);
        eng.add_generated_card_to_combat_at(dazed, PileType::Discard, None, Position::Bottom)
    });
    assert_eq!(c.state.player.orbs.len(), before);
    with_engine(&mut c, |eng| {
        let dazed = eng.create_card(CardKind::Dazed);
        eng.add_generated_card_to_combat_at(dazed, PileType::Discard, Some(p), Position::Bottom)
    });
    let kinds: Vec<OrbKind> = c.state.player.orbs.iter().map(|o| o.kind).collect();
    assert_eq!(kinds.len(), 3);
    assert_eq!(
        kinds[before - 1..],
        [OrbKind::Lightning, expected[0], expected[1]][..]
    );
}
