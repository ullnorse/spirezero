//! The gold glow of hand cards (`CardModel.ShouldGlowGold`, `NHandCardHolder.ShouldGlowGold`):
//! each card's `ShouldGlowGoldInternal`, met and not met (paths under
//! `ref/decomp/MegaCrit.Sts2.Core.`).

use super::*;

/// Moves `card` from its pile to the end of `pile`.
fn move_card(s: &mut CombatState, card: CardId, pile: PileType) {
    s.remove_from_pile(card);
    s.insert_into_pile(card, pile, None);
}

/// A combat in its Play phase whose hand holds only a fresh card of `kind`.
fn alone_in_hand(kind: CardKind) -> (Combat, CardId) {
    let mut c = new_combat();
    let hand = c.state.player.hand.clone();
    c.edit(|s| {
        for card in hand {
            move_card(s, card, PileType::Discard);
        }
    });
    let card = give_card(&mut c, kind, 0);
    assert_eq!(c.state.player.phase, TurnPhase::Play);
    (c, card)
}

fn glows(c: &Combat, card: CardId) -> bool {
    c.state.glows_gold(card)
}

#[test]
fn only_playable_cards_in_the_play_phase_glow() {
    // NHandCardHolder.ShouldGlowGold: `CanPlay()` and `Phase == Play` first.
    let (mut c, card) = alone_in_hand(CardKind::Restlessness);
    assert!(glows(&c, card));
    c.state.player.phase = TurnPhase::End;
    assert!(!glows(&c, card));
    let (mut c, card) = alone_in_hand(CardKind::EvilEye);
    with_engine(&mut c, |eng| {
        let other = eng.s.player.draw[0];
        eng.exhaust(other, false)
    });
    assert!(glows(&c, card));
    c.state.player.energy = 0;
    assert!(!glows(&c, card), "Evil Eye costs 1");
}

#[test]
fn cards_without_a_condition_never_glow() {
    let (c, card) = alone_in_hand(CardKind::StrikeNecrobinder);
    assert!(!glows(&c, card));
}

#[test]
fn heavenly_drill_glows_at_its_energy_var() {
    let (mut c, card) = alone_in_hand(CardKind::HeavenlyDrill);
    c.state.player.energy = 3;
    assert!(!glows(&c, card));
    c.state.player.energy = 4;
    assert!(glows(&c, card));
}

#[test]
fn clash_glows_when_playable() {
    let (mut c, card) = alone_in_hand(CardKind::Clash);
    assert!(glows(&c, card));
    give_card(&mut c, CardKind::DefendNecrobinder, 0);
    assert!(!glows(&c, card), "a Skill in hand makes it unplayable");
}

#[test]
fn grand_finale_glows_with_an_empty_draw_pile() {
    let (mut c, card) = alone_in_hand(CardKind::GrandFinale);
    assert!(!glows(&c, card));
    let draw = c.state.player.draw.clone();
    c.edit(|s| {
        for card in draw {
            move_card(s, card, PileType::Discard);
        }
    });
    assert!(glows(&c, card));
}

#[test]
fn deaths_door_glows_after_the_player_applies_doom() {
    let (mut c, card) = alone_in_hand(CardKind::DeathsDoor);
    let (p, e) = (player(&c), enemy(&c));
    assert!(!glows(&c, card));
    with_engine(&mut c, |eng| {
        eng.apply_power(e, PowerKind::Doom, Decimal::from(3), Some(e))
    });
    assert!(!glows(&c, card), "Doom from another applier");
    with_engine(&mut c, |eng| {
        eng.apply_power(e, PowerKind::Doom, Decimal::from(3), Some(p))
    });
    assert!(glows(&c, card));
    c.state.round += 1;
    assert!(!glows(&c, card), "only this turn");
}

#[test]
fn flatten_glows_after_osty_attacks() {
    let (mut c, card) = alone_in_hand(CardKind::Flatten);
    let (o, e) = (osty(&c), enemy(&c));
    assert!(!glows(&c, card));
    with_engine(&mut c, |eng| {
        eng.attack(&commands::Attack::new(
            o,
            commands::AttackTargets::Single(e),
            commands::AttackDamage::Fixed(Decimal::ONE),
            None,
        ))
        .map(drop)
    });
    assert!(glows(&c, card));
}

#[test]
fn power_conditions_on_hittable_enemies() {
    // Dismantle: Vulnerable; Bubble Bubble: Poison.
    for (kind, power) in [
        (CardKind::Dismantle, PowerKind::Vulnerable),
        (CardKind::BubbleBubble, PowerKind::Poison),
    ] {
        let (mut c, card) = alone_in_hand(kind);
        let (p, e) = (player(&c), enemy(&c));
        assert!(!glows(&c, card), "{kind:?}");
        with_engine(&mut c, |eng| {
            eng.apply_power(p, power, Decimal::from(2), Some(p))
        });
        assert!(!glows(&c, card), "{kind:?}: only enemies count");
        with_engine(&mut c, |eng| {
            eng.apply_power(e, power, Decimal::from(2), Some(p))
        });
        assert!(glows(&c, card), "{kind:?}");
    }
}

#[test]
fn go_for_the_eyes_glows_when_an_enemy_intends_to_attack() {
    let (c, card) = alone_in_hand(CardKind::GoForTheEyes);
    let attacks = c
        .state
        .hittable_enemies()
        .iter()
        .any(|&e| c.state.intends_to_attack(e));
    assert!(attacks, "the test fight opens with an attack");
    assert!(glows(&c, card));
    let (mut c, card) = alone_in_hand(CardKind::GoForTheEyes);
    c.edit(|s| {
        for i in 0..s.enemies.len() {
            let e = s.enemies[i];
            s.creature_mut(e).hp = 0;
        }
    });
    assert!(!glows(&c, card), "no hittable enemy");
}

#[test]
fn pacts_end_glows_with_three_exhausted_cards() {
    let (mut c, card) = alone_in_hand(CardKind::PactsEnd);
    for n in 0..3 {
        assert!(!glows(&c, card), "{n} exhausted");
        with_engine(&mut c, |eng| {
            let other = eng.s.player.draw[0];
            eng.exhaust(other, false)
        });
    }
    assert!(glows(&c, card));
}

#[test]
fn impatience_glows_without_an_attack_in_hand() {
    let (mut c, card) = alone_in_hand(CardKind::Impatience);
    assert!(glows(&c, card));
    give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    assert!(!glows(&c, card));
}

#[test]
fn restlessness_glows_alone_in_hand() {
    let (mut c, card) = alone_in_hand(CardKind::Restlessness);
    assert!(glows(&c, card));
    give_card(&mut c, CardKind::DefendNecrobinder, 0);
    assert!(!glows(&c, card));
}

#[test]
fn spite_glows_after_hp_loss_this_turn() {
    let (mut c, card) = alone_in_hand(CardKind::Spite);
    let (p, e) = (player(&c), enemy(&c));
    // Osty takes hits for the player; with it dead, they reach the player.
    let o = osty(&c);
    with_engine(&mut c, |eng| eng.kill(&[o]));
    assert!(!glows(&c, card));
    with_engine(&mut c, |eng| {
        eng.damage(&[p], Decimal::from(3), ValueProps::MOVE, Some(e), None)
            .map(drop)
    });
    assert!(glows(&c, card));
}

#[test]
fn ftl_and_fetch_glow_by_this_turns_plays() {
    // FTL: fewer than PlayMax (3) finished plays this turn; Fetch: not yet played this turn.
    let (mut c, ftl) = alone_in_hand(CardKind::Ftl);
    let fetch = give_card(&mut c, CardKind::Fetch, 0);
    let e = enemy(&c);
    for n in 0..3 {
        assert!(glows(&c, ftl), "{n} plays");
        let card = if n == 0 {
            fetch
        } else {
            give_card(&mut c, CardKind::Fetch, 0)
        };
        c.apply(Action::PlayCard {
            card,
            target: Some(e),
        })
        .expect("Fetch is playable");
    }
    assert!(!glows(&c, ftl));
    // The first Fetch is back in the hand only through the discard; its play still counts.
    let again = c.state.player.discard.iter().copied().find(|&x| x == fetch);
    assert_eq!(again, Some(fetch));
    c.edit(|s| move_card(s, fetch, PileType::Hand));
    assert!(!glows(&c, fetch));
    let fresh = give_card(&mut c, CardKind::Fetch, 0);
    assert!(glows(&c, fresh));
}
