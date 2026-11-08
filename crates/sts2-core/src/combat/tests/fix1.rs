//! A card that asks for a card from the hand or a pile with nothing there to choose
//! (`CardSelectCmd.FromHand`, `CardSelectCmd.cs:834-843`: an empty list is the result, with no
//! selection) resolves at once, and the turn goes on (FIX1).

use super::*;

/// Every card out of the hand, the draw pile and the discard pile (into the exhaust pile).
fn bare(c: &mut Combat) {
    c.edit(|s| {
        let cards: Vec<CardId> = [&s.player.hand, &s.player.draw, &s.player.discard]
            .into_iter()
            .flat_map(|pile| pile.iter().copied())
            .collect();
        for card in cards {
            s.remove_from_pile(card);
            s.insert_into_pile(card, PileType::Exhaust, None);
        }
    });
}

/// Plays `kind` (at the first enemy when it aims) with nothing else in the hand or the piles,
/// and checks no choice opened and End Turn is there.
fn plays_through(kind: CardKind, upgrade_level: u8) -> Combat {
    let mut c = new_combat();
    bare(&mut c);
    let card = give_card(&mut c, kind, upgrade_level);
    let target = c
        .legal_actions()
        .into_iter()
        .find_map(|a| match a {
            Action::PlayCard { card: id, target } if id == card => Some(target),
            _ => None,
        })
        .expect("the card is playable");
    assert_eq!(
        c.apply(Action::PlayCard { card, target }),
        Ok(Status::PlayerTurn),
        "{kind:?}: no choice with nothing to choose"
    );
    assert_eq!(c.pending_decision(), None, "{kind:?}");
    assert!(c.legal_actions().contains(&Action::EndTurn), "{kind:?}");
    c
}

#[test]
fn survivor_with_an_empty_hand_discards_nothing() {
    // Survivor.cs: GainBlock, then FromHandForDiscard(.., 1); the empty hand selects nothing and
    // `FirstOrDefault()` is null, so nothing is discarded.
    let c = plays_through(CardKind::Survivor, 0);
    assert_eq!(c.state.creature(player(&c)).block, 8);
    assert_eq!(c.state.player.discard.len(), 1, "only Survivor itself");
}

#[test]
fn survivor_with_one_card_discards_it_without_asking() {
    // CardSelectCmd.cs:840: not RequireManualConfirmation and Count <= MinSelect: that card.
    let mut c = new_combat();
    bare(&mut c);
    let other = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    let survivor = give_card(&mut c, CardKind::Survivor, 0);
    assert_eq!(
        c.apply(Action::PlayCard {
            card: survivor,
            target: None
        }),
        Ok(Status::PlayerTurn)
    );
    assert_eq!(c.state.pile_of(other), Some(PileType::Discard));
    assert!(c.state.player.hand.is_empty());
}

#[test]
fn hand_choices_with_an_empty_hand_ask_nothing() {
    // Acrobatics.cs, DaggerThrow.cs, Prepared.cs, HiddenDaggers.cs: draw (nothing to draw),
    // then FromHandForDiscard. Armaments.cs: FromHandForUpgrade
    // (`CardSelectCmd.cs:927`: one candidate or none, no selection). TrueGrit.cs (upgraded) and
    // BurningPact.cs: FromHand to exhaust. Purity.cs: FromHand, 0 to N.
    for (kind, upgrade_level) in [
        (CardKind::Acrobatics, 0),
        (CardKind::DaggerThrow, 0),
        (CardKind::Prepared, 0),
        (CardKind::HiddenDaggers, 0),
        (CardKind::Armaments, 0),
        (CardKind::TrueGrit, 1),
        (CardKind::BurningPact, 0),
        (CardKind::Purity, 0),
    ] {
        plays_through(kind, upgrade_level);
    }
}

#[test]
fn pile_choices_with_an_empty_pile_ask_nothing() {
    // Headbutt.cs, Dredge.cs: FromCombatPile over the discard pile; an empty pile gives no
    // selection (`CardSelectCmd.cs:477-480`).
    for kind in [CardKind::Headbutt, CardKind::Dredge] {
        plays_through(kind, 0);
    }
}
