//! Shivs, the dynamic Shiv target, single-turn Retain/Sly and `TheHunt`'s extra reward (C19;
//! paths under `ref/decomp/MegaCrit.Sts2.Core.`, spec `docs/specs/silent.md`).

use super::*;
use crate::content::cards::shiv;

/// A Silent combat against two Slimes on turn 1.
fn silent_combat() -> Combat {
    Combat::new(
        CombatConfig::new(Character::Silent, SEED, 0)
            .expect("Silent's starter is ported")
            .with_encounter(Encounter::SlimesWeak),
    )
}

/// Moves the whole hand to the discard pile (the opening hand would hit the hand cap).
fn empty_hand(c: &mut Combat) {
    c.edit(|s| {
        while let Some(&card) = s.player.hand.first() {
            s.remove_from_pile(card);
            s.insert_into_pile(card, PileType::Discard, None);
        }
    });
}

fn hand_shivs(c: &Combat) -> Vec<CardId> {
    c.state
        .player
        .hand
        .iter()
        .copied()
        .filter(|&card| c.state.card(card).kind.is_shiv())
        .collect()
}

#[test]
fn only_shivs_are_tagged_shiv() {
    // Shiv.cs:25 (CanonicalTags).
    assert!(CardKind::Shiv.is_shiv());
    assert!(!CardKind::StrikeSilent.is_shiv());
}

#[test]
fn shiv_create_in_hand_adds_cards_and_logs_them() {
    // Shiv.cs:97-119 (CreateInHand): `count` CreateCard, then AddGeneratedCardsToCombat(Hand).
    let mut c = silent_combat();
    let generated = c.state.history_count(HistoryTag::CardGenerated);
    let none = with_engine(&mut c, |eng| shiv::create_in_hand(eng, 0));
    assert_eq!(none.len(), 0);
    let shivs = with_engine(&mut c, |eng| shiv::create_in_hand(eng, 3));
    assert_eq!(shivs.len(), 3);
    assert!(
        shivs
            .iter()
            .all(|&s| c.state.pile_of(s) == Some(PileType::Hand))
    );
    assert_eq!(
        c.state.history_count(HistoryTag::CardGenerated),
        generated + 3
    );
}

#[test]
fn shivs_created_over_the_hand_cap_do_not_enter_the_hand() {
    // CardPileCmd.cs: the hand holds 10 cards (the rest of a generated batch is lost).
    let mut c = silent_combat();
    empty_hand(&mut c);
    let shivs = with_engine(&mut c, |eng| shiv::create_in_hand(eng, 12));
    assert_eq!(shivs.len(), 12);
    assert_eq!(c.state.player.hand.len(), 10);
}

#[test]
fn a_shiv_hits_one_enemy_for_four_or_six_upgraded_and_exhausts() {
    // Shiv.cs:55-72 (OnPlay), 76 (UpgradeValueBy(2)), 27 (Exhaust).
    let mut c = silent_combat();
    let plain = give_card(&mut c, CardKind::Shiv, 0);
    let upgraded = give_card(&mut c, CardKind::Shiv, 1);
    let (a, b) = (c.state.enemies[0], c.state.enemies[1]);
    let (hp_a, hp_b) = (c.state.creature(a).hp, c.state.creature(b).hp);
    c.apply(Action::PlayCard {
        card: plain,
        target: Some(a),
    })
    .expect("legal");
    assert_eq!(c.state.creature(a).hp, hp_a - 4);
    assert_eq!(c.state.creature(b).hp, hp_b);
    assert_eq!(c.state.pile_of(plain), Some(PileType::Exhaust));
    c.apply(Action::PlayCard {
        card: upgraded,
        target: Some(a),
    })
    .expect("legal");
    assert_eq!(c.state.creature(a).hp, hp_a - 10);
}

#[test]
fn fan_of_knives_makes_shivs_target_and_hit_every_enemy() {
    // Shiv.cs:14-22 (TargetType), 59-63 (TargetingAllOpponents); FanOfKnivesPower.cs.
    let mut c = silent_combat();
    empty_hand(&mut c);
    let shiv = give_card(&mut c, CardKind::Shiv, 0);
    assert_eq!(c.state.target_type(shiv), TargetType::AnyEnemy);
    let targeted = c
        .legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::PlayCard { card, target: Some(_) } if *card == shiv))
        .count();
    assert_eq!(targeted, c.state.enemies.len());
    c.state.player.energy = 2;
    let fan = give_card(&mut c, CardKind::FanOfKnives, 0);
    c.apply(Action::PlayCard {
        card: fan,
        target: None,
    })
    .expect("legal");
    assert_eq!(hand_shivs(&c).len(), 5, "the given Shiv and 4 new ones");
    assert_eq!(c.state.target_type(shiv), TargetType::AllEnemies);
    let plays: Vec<Action> = c
        .legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::PlayCard { card, .. } if *card == shiv))
        .collect();
    assert_eq!(
        plays,
        [Action::PlayCard {
            card: shiv,
            target: None
        }]
    );
    let hps: Vec<i32> = c
        .state
        .enemies
        .iter()
        .map(|&e| c.state.creature(e).hp)
        .collect();
    c.apply(Action::PlayCard {
        card: shiv,
        target: None,
    })
    .expect("no target needed");
    for (&e, hp) in c.state.enemies.iter().zip(hps) {
        assert_eq!(c.state.creature(e).hp, hp - 4);
    }
}

#[test]
fn blade_dance_and_cloak_and_dagger_create_shivs() {
    // BladeDance.cs (CardsVar(3), +1), CloakAndDagger.cs (BlockVar(6), CardsVar(1), +1 Cards).
    let mut c = silent_combat();
    empty_hand(&mut c);
    c.state.player.energy = 9;
    let dance = give_card(&mut c, CardKind::BladeDance, 1);
    let cloak = give_card(&mut c, CardKind::CloakAndDagger, 1);
    let block = c.state.creature(player(&c)).block;
    c.apply(Action::PlayCard {
        card: dance,
        target: None,
    })
    .expect("legal");
    assert_eq!(hand_shivs(&c).len(), 4);
    c.apply(Action::PlayCard {
        card: cloak,
        target: None,
    })
    .expect("legal");
    assert_eq!(hand_shivs(&c).len(), 6);
    assert_eq!(c.state.creature(player(&c)).block, block + 6);
}

#[test]
fn single_turn_sly_and_retain_commands_set_the_fields_and_end_of_turn_clears_them() {
    // CardCmd.cs:702-715; CardModel.cs:1348-1360; EndOfTurnCleanup.
    let mut c = silent_combat();
    let card = give_card(&mut c, CardKind::DefendSilent, 0);
    assert!(!c.state.is_sly_this_turn(card));
    with_engine(&mut c, |eng| {
        eng.apply_single_turn_sly(card);
        eng.apply_single_turn_retain(card);
        Ok(())
    });
    assert!(c.state.is_sly_this_turn(card));
    c.apply(Action::EndTurn).expect("legal");
    assert!(c.state.player.hand.contains(&card), "retained");
    assert!(!c.state.is_sly_this_turn(card));
    assert!(!c.state.card(card).single_turn_retain);
}

#[test]
fn expertise_retains_the_cards_it_draws() {
    // Expertise.cs:19-25: draws CardsVar(2), each gets single-turn Retain.
    let mut c = silent_combat();
    c.state.player.energy = 3;
    let before: Vec<CardId> = c.state.player.hand.to_vec();
    let expertise = give_card(&mut c, CardKind::Expertise, 0);
    c.apply(Action::PlayCard {
        card: expertise,
        target: None,
    })
    .expect("legal");
    let drawn: Vec<CardId> = c
        .state
        .player
        .hand
        .iter()
        .copied()
        .filter(|h| !before.contains(h))
        .collect();
    assert_eq!(drawn.len(), 2);
    assert!(drawn.iter().all(|&d| c.state.card(d).single_turn_retain));
    assert!(before.iter().all(|&b| !c.state.card(b).single_turn_retain));
}

#[test]
fn the_hunt_adds_a_card_reward_only_when_it_kills() {
    // TheHunt.cs:33-45; CombatRoom.cs:185 (AddExtraReward).
    let mut c = silent_combat();
    c.state.player.energy = 9;
    let (a, b) = (c.state.enemies[0], c.state.enemies[1]);
    c.state.creature_mut(a).hp = 100;
    c.state.creature_mut(b).hp = 1;
    let hunt = give_card(&mut c, CardKind::TheHunt, 0);
    c.apply(Action::PlayCard {
        card: hunt,
        target: Some(a),
    })
    .expect("legal");
    assert_eq!(c.state.extra_rewards.len(), 0);
    let again = give_card(&mut c, CardKind::TheHunt, 0);
    c.apply(Action::PlayCard {
        card: again,
        target: Some(b),
    })
    .expect("legal");
    assert_eq!(c.state.extra_rewards, [ExtraReward::Card(3)]);
    assert_eq!(
        c.state
            .creature(player(&c))
            .power_amount(PowerKind::TheHunt),
        1
    );
}
