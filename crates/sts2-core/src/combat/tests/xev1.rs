//! Hive event batch EV1: the event cards, relics and enchantment in combat (paths under
//! `Models.Cards/`, `Models.Relics/`, `Models.Enchantments/`).

use super::*;
use crate::combat::commands::Position;
use crate::content::cards::CardType;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::{OwnedRelic, RelicData, pollinous_core};

fn with_relic(relic: OwnedRelic) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.relics.push(relic);
    Combat::new(config)
}

fn play(c: &mut Combat, card: CardId, target: Option<CreatureId>) {
    c.apply(Action::PlayCard { card, target }).expect("legal");
}

#[test]
fn enlightenment_caps_the_hand_at_one_this_turn_or_this_combat() {
    // Enlightenment.cs: `SetThisTurnOrUntilPlayed(1, reduceOnly: true)`, upgraded
    // `SetThisCombat`; a cheaper card keeps its cost.
    for upgrade in [0, 1] {
        let mut c = new_combat();
        let costly = give_card(&mut c, CardKind::Metamorphosis, 0);
        let cheap = give_card(&mut c, CardKind::Squash, 0);
        let free = give_card(&mut c, CardKind::Enlightenment, 0);
        let enlightenment = give_card(&mut c, CardKind::Enlightenment, upgrade);
        play(&mut c, enlightenment, None);
        assert_eq!(c.state.energy_cost(costly), 1);
        assert_eq!(c.state.energy_cost(cheap), 1);
        assert_eq!(c.state.energy_cost(free), 0, "reduce only");
        let expected = if upgrade == 0 { 2 } else { 1 };
        c.state.end_of_turn_cleanup_of(costly);
        assert_eq!(c.state.energy_cost(costly), expected, "upgrade {upgrade}");
    }
}

#[test]
fn exterminate_hits_every_enemy_four_times() {
    let mut c = new_combat();
    let target = enemy(&c);
    let hp = c.state.creature(target).hp;
    let card = give_card(&mut c, CardKind::Exterminate, 0);
    play(&mut c, card, None);
    assert_eq!(c.state.creature(target).hp, hp - 12);
}

#[test]
fn squash_hits_and_applies_vulnerable() {
    let mut c = new_combat();
    let target = enemy(&c);
    let hp = c.state.creature(target).hp;
    let card = give_card(&mut c, CardKind::Squash, 1);
    play(&mut c, card, Some(target));
    assert_eq!(c.state.creature(target).hp, hp - 12);
    assert_eq!(
        c.state.creature(target).power_amount(PowerKind::Vulnerable),
        3
    );
}

#[test]
fn metamorphosis_shuffles_free_attacks_into_the_draw_pile() {
    // Metamorphosis.cs: 3 (5 upgraded) pool Attacks, `SetToFreeThisCombat`, draw pile at random.
    let mut c = new_combat();
    let draw = c.state.player.draw.len();
    let card = give_card(&mut c, CardKind::Metamorphosis, 1);
    play(&mut c, card, None);
    assert_eq!(c.state.player.draw.len(), draw + 5);
    let new: Vec<CardId> = c
        .state
        .player
        .draw
        .iter()
        .copied()
        .filter(|&id| c.state.card(id).deck_version().is_none())
        .collect();
    assert_eq!(new.len(), 5);
    for id in new {
        assert_eq!(c.state.card(id).card_type(), CardType::Attack);
        assert_eq!(c.state.energy_cost(id), 0);
    }
}

#[test]
fn perfect_fit_tops_the_draw_pile_after_a_reshuffle() {
    // PerfectFit.cs: `ModifyShuffleOrder` moves the card to index 0 unless it is the first
    // shuffle (`CardPileCmd.Shuffle` passes `isInitialShuffle: false`).
    let mut c = new_combat();
    let card = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    with_engine(&mut c, |eng| {
        eng.enchant(card, EnchantmentKind::PerfectFit, Decimal::ONE);
        let hand = eng.s.player.hand.to_vec();
        let draw = eng.s.player.draw.to_vec();
        eng.add_cards_to_pile(&hand, PileType::Discard, Position::Bottom)?;
        eng.add_cards_to_pile(&draw, PileType::Discard, Position::Bottom)?;
        eng.shuffle()
    });
    assert_eq!(c.state.player.draw.first(), Some(&card));
}

#[test]
fn lost_wisp_hits_every_enemy_when_a_power_is_played() {
    // LostWisp.cs: 8 unpowered damage to the hittable enemies after a Power is played.
    let mut c = with_relic(OwnedRelic::from(RelicKind::LostWisp));
    let target = enemy(&c);
    let hp = c.state.creature(target).hp;
    let skill = give_card(&mut c, CardKind::Enlightenment, 0);
    play(&mut c, skill, None);
    assert_eq!(c.state.creature(target).hp, hp);
    let power = give_card(&mut c, CardKind::Accuracy, 0);
    play(&mut c, power, None);
    assert_eq!(c.state.creature(target).hp, hp - 8);
}

#[test]
fn pollinous_core_draws_two_more_on_its_fourth_turn() {
    // PollinousCore.cs: `TurnsSeen++` before the hand draw; at 4 the draw is 2 more and
    // `AfterModifyingHandDraw` resets it to 0. The count is saved between combats.
    let c = with_relic(OwnedRelic {
        kind: RelicKind::PollinousCore,
        data: RelicData::PollinousCore(pollinous_core::Data { turns_seen: 3 }),
        melted: false,
    });
    assert_eq!(c.state.player.hand.len(), 7);
    let RelicData::PollinousCore(data) = c.state.relic(RelicId(1)).data else {
        panic!("not a PollinousCore");
    };
    assert_eq!(data.turns_seen, 0);
    let c = with_relic(OwnedRelic::from(RelicKind::PollinousCore));
    assert_eq!(c.state.player.hand.len(), 5);
    let RelicData::PollinousCore(data) = c.state.relic(RelicId(1)).data else {
        panic!("not a PollinousCore");
    };
    assert_eq!(data.turns_seen, 1);
}
