//! Relic instance data, energy and block loss, autoplay from the draw pile, combat card
//! generation and the choose-a-card screen (C9, `docs/specs/content-gaps.md` A1-A5).

use super::*;
use crate::content::potions::PotionKind;
use crate::content::relics::{OwnedRelic, RelicData, nunchaku};

fn with_relic(seed: &str, relic: impl Into<OwnedRelic>) -> Combat {
    let mut config = CombatConfig::necrobinder(seed, 0);
    config.relics.push(relic.into());
    Combat::new(config)
}

fn attacks_played(c: &Combat, relic: RelicId) -> i32 {
    let RelicData::Nunchaku(data) = c.state.relic(relic).data else {
        panic!("not a Nunchaku");
    };
    data.attacks_played
}

#[test]
fn nunchaku_counts_attacks_from_its_saved_data() {
    // Nunchaku.cs: AfterCardPlayed counts the owner's Attacks; every 10th gains 1 energy. The
    // count comes in with the relic's data (the run keeps it between combats).
    let nunchaku = OwnedRelic {
        kind: RelicKind::Nunchaku,
        data: RelicData::Nunchaku(nunchaku::Data { attacks_played: 8 }),
        melted: false,
    };
    let mut c = with_relic(SEED, nunchaku);
    let me = RelicId(1);
    let energy = c.state.player.energy;
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    c.apply(Action::PlayCard {
        card: defend,
        target: None,
    })
    .expect("legal");
    assert_eq!(attacks_played(&c, me), 8, "a Skill does not count");
    let target = Some(enemy(&c));
    for (played, expected_energy) in [(9, energy - 2), (10, energy - 2)] {
        let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
        c.apply(Action::PlayCard {
            card: strike,
            target,
        })
        .expect("legal");
        assert_eq!(attacks_played(&c, me), played);
        assert_eq!(c.state.player.energy, expected_energy);
    }
}

#[test]
fn bread_loses_energy_on_turn_one_and_raises_max_energy_later() {
    // Bread.cs, PlayerCmd.cs:49-61: LoseEnergy(2) after turn 1 starts; ModifyMaxEnergy +1 from
    // turn 2.
    let mut c = with_relic(SEED, RelicKind::Bread);
    assert_eq!(c.state.player.energy, 1);
    assert_eq!(c.state.max_energy(), 3);
    c.apply(Action::EndTurn).expect("legal");
    assert_eq!(c.state.player.turn_number, 2);
    assert_eq!(c.state.player.energy, 4);
}

#[test]
fn lose_block_truncates_and_stops_at_zero() {
    // CreatureCmd.cs:710-722, Creature.cs:469-476.
    let mut c = new_combat();
    let p = player(&c);
    c.state.creature_mut(p).block = 15;
    with_engine(&mut c, |eng| eng.lose_block(p, Decimal::new(55, 1), None));
    assert_eq!(c.state.creature(p).block, 9);
    with_engine(&mut c, |eng| eng.lose_block(p, Decimal::ZERO, None));
    assert_eq!(c.state.creature(p).block, 9);
    with_engine(&mut c, |eng| eng.lose_block(p, Decimal::from(20), None));
    assert_eq!(c.state.creature(p).block, 0);
}

#[test]
fn sturdy_clamp_keeps_ten_block() {
    // SturdyClamp.cs: ShouldClearBlock vetoes for the owner; AfterPreventingBlockClear loses
    // all but 10.
    for (relic, kept) in [(Some(RelicKind::SturdyClamp), 10), (None, 0)] {
        let mut config = CombatConfig::necrobinder(SEED, 0);
        config.relics.extend(relic.map(OwnedRelic::from));
        let mut c = Combat::new(config);
        let p = player(&c);
        c.state.creature_mut(p).block = 200;
        c.apply(Action::EndTurn).expect("legal");
        assert_eq!(c.state.player.turn_number, 2);
        assert_eq!(c.state.creature(p).block, kept);
    }
}

#[test]
fn distilled_chaos_auto_plays_the_top_of_the_draw_pile() {
    // DistilledChaos.cs, CardPileCmd.cs:1148-1183: the top 3 cards go to the Play pile, then
    // each is auto-played for free.
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.potions = vec![Some(PotionKind::DistilledChaos.into())];
    let mut c = Combat::new(config);
    let top: Vec<CardId> = c.state.player.draw[..3].to_vec();
    let energy = c.state.player.energy;
    c.apply(Action::UsePotion {
        slot: 0,
        target: None,
    })
    .expect("legal");
    assert_eq!(c.state.player.energy, energy);
    for card in top {
        assert_eq!(c.state.pile_of(card), Some(PileType::Discard));
    }
    assert_eq!(c.state.history_count(HistoryTag::CardPlayStarted), 3);
    assert_eq!(c.state.player.draw.len(), 10 - 5 - 3);
}

#[test]
fn big_hat_generates_distinct_ethereal_pool_cards() {
    // BigHat.cs, CardFactory.cs:119-162: on D1S0, the UnstableShuffle of the 8 Ethereal pool
    // cards on CombatCardGeneration starts with Seance and Defile.
    let c = with_relic("D1S0", RelicKind::BigHat);
    let hand: Vec<CardKind> = c
        .state
        .player
        .hand
        .iter()
        .map(|&h| c.state.card(h).kind)
        .collect();
    assert_eq!(hand.len(), 7);
    assert_eq!(&hand[5..], [CardKind::Seance, CardKind::Defile]);
}

#[test]
fn choose_a_card_offers_generated_cards_and_may_be_skipped() {
    // CardSelectCmd.cs:252-267: the selector picks 0 to 1 of the cards; only the pick enters
    // combat.
    let mut c = with_relic(SEED, RelicKind::TestChooseACard);
    let Some(DecisionRequest::ChooseCards {
        options,
        min: 0,
        max: 1,
        ..
    }) = c.pending_decision().cloned()
    else {
        panic!("expected a choose-a-card prompt");
    };
    assert_eq!(options.len(), 3);
    assert!(options.iter().all(|&o| c.state.pile_of(o).is_none()));
    let mut skipped = c.clone();
    c.apply(Action::Resolve(Decision::Pick(options[1])))
        .expect("legal");
    assert_eq!(c.state.pile_of(options[1]), Some(PileType::Hand));
    assert_eq!(c.state.player.hand.len(), 6);
    assert_eq!(c.state.pile_of(options[0]), None);
    skipped
        .apply(Action::Resolve(Decision::Done))
        .expect("legal");
    assert_eq!(skipped.state.player.hand.len(), 5);
}

#[test]
fn a_picked_card_goes_back_before_the_choice_is_confirmed() {
    // GamblersBrew.cs, NPlayerHand.DeselectCard: a selected card returns to the hand until the
    // selection is confirmed; the unpick is a client's answer, never in the legal actions.
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.potions = vec![Some(PotionKind::GamblersBrew.into())];
    let mut c = Combat::new(config);
    c.apply(Action::UsePotion {
        slot: 0,
        target: Some(player(&c)),
    })
    .expect("legal");
    let hand: Vec<CardId> = c.state.player.hand.to_vec();
    let (a, b) = (hand[0], hand[1]);
    assert_eq!(
        c.apply(Action::Resolve(Decision::Unpick(a))),
        Err(ActionError::InvalidDecision)
    );
    c.apply(Action::Resolve(Decision::Pick(a))).expect("legal");
    c.apply(Action::Resolve(Decision::Pick(b))).expect("legal");
    let Some(request) = c.pending_decision() else {
        panic!("still choosing");
    };
    assert_eq!(
        request.unpicks(),
        [Decision::Unpick(a), Decision::Unpick(b)]
    );
    assert!(
        !c.legal_actions()
            .contains(&Action::Resolve(Decision::Unpick(a)))
    );
    assert_eq!(
        c.apply(Action::Resolve(Decision::Unpick(a))),
        Ok(Status::Decision)
    );
    assert_eq!(
        c.apply(Action::Resolve(Decision::Unpick(a))),
        Err(ActionError::InvalidDecision)
    );
    assert!(
        c.legal_actions()
            .contains(&Action::Resolve(Decision::Pick(a)))
    );
    assert_eq!(
        c.apply(Action::Resolve(Decision::Done)),
        Ok(Status::PlayerTurn)
    );
    assert_eq!(c.state.pile_of(b), Some(PileType::Discard));
    assert_eq!(c.state.pile_of(a), Some(PileType::Hand));
    assert_eq!(c.state.player.hand.len(), 5);
}
