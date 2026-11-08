//! Card-class result locations, `CardModel.GetResultLocationForCardPlay` (C21; paths under
//! `ref/decomp/MegaCrit.Sts2.Core.`).

use super::*;

/// A Regent combat with an empty hand and `stars` stars.
fn regent_combat(stars: i32) -> Combat {
    let mut c = Combat::new(
        CombatConfig::new(Character::Regent, SEED, 0).expect("Regent's starter is ported"),
    );
    for card in c.state.player.hand.clone() {
        c.state.remove_from_pile(card);
        c.state.insert_into_pile(card, PileType::Discard, None);
    }
    c.state.player.stars = stars;
    c
}

fn play(c: &mut Combat, card: CardId, target: Option<CreatureId>) {
    c.apply(Action::PlayCard { card, target }).expect("legal");
}

#[test]
fn shining_strike_goes_on_top_of_the_draw_pile() {
    // Models.Cards/ShiningStrike.cs: a Discard result becomes Draw, Top.
    let mut c = regent_combat(0);
    let e = enemy(&c);
    let strike = give_card(&mut c, CardKind::ShiningStrike, 1);
    play(&mut c, strike, Some(e));
    assert_eq!(c.state.player.draw.first(), Some(&strike));
    assert_eq!(c.state.player.stars, 2);
}

#[test]
fn particle_wall_returns_to_the_hand() {
    // Models.Cards/ParticleWall.cs: a Discard result becomes Hand.
    let mut c = regent_combat(2);
    let wall = give_card(&mut c, CardKind::ParticleWall, 0);
    play(&mut c, wall, None);
    assert_eq!(c.state.pile_of(wall), Some(PileType::Hand));
    assert_eq!(c.state.creature(player(&c)).block, 9);
    assert_eq!(c.state.player.stars, 0);
}

#[test]
fn particle_wall_is_discarded_into_a_full_hand() {
    // Commands/CardPileCmd.cs:484-488: an add to a full hand goes to the discard pile.
    let mut c = regent_combat(2);
    let wall = give_card(&mut c, CardKind::ParticleWall, 0);
    // `CardPile.MaxCardsInHand` other cards.
    for _ in 0..10 {
        give_card(&mut c, CardKind::DefendRegent, 0);
    }
    play(&mut c, wall, None);
    assert_eq!(c.state.pile_of(wall), Some(PileType::Discard));
}

#[test]
fn corruption_still_exhausts_particle_wall() {
    // CardModel.cs:1881-1882: the card's location is computed first, then
    // ModifyCardPlayResultLocation (CorruptionPower) overrides it.
    let mut c = regent_combat(2);
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Corruption, Decimal::ONE, Some(p))
    });
    let wall = give_card(&mut c, CardKind::ParticleWall, 0);
    play(&mut c, wall, None);
    assert_eq!(c.state.pile_of(wall), Some(PileType::Exhaust));
}
