//! Stars, star costs, Forge and Sovereign Blade, `VoidForm`'s end of turn and the combat's extra
//! rewards (C17; paths under `ref/decomp/MegaCrit.Sts2.Core.`, spec `docs/specs/regent.md`).

use super::*;
use crate::content::cards::sovereign_blade;
use history::HistoryTag;

/// A Regent combat on turn 1, after `DivineRight` gained its 3 stars.
fn regent_combat() -> Combat {
    Combat::new(
        CombatConfig::new(Character::Regent, SEED, 0)
            .expect("Regent's starter is ported")
            .with_events(),
    )
}

fn stars(c: &Combat) -> i32 {
    c.state.player.stars
}

#[test]
fn divine_right_gains_three_stars_and_logs_the_change() {
    // DivineRight.cs:16, PlayerCombatState.cs:103-117 (StarsModified on every change).
    let c = regent_combat();
    assert_eq!(stars(&c), 3);
    let entries: Vec<_> = c
        .state
        .history_this_turn(HistoryTag::StarsModified)
        .iter()
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, HistoryKind::StarsModified(3));
}

#[test]
fn a_star_cost_is_paid_at_play_and_gates_legality() {
    // CardModel.cs:1807-1845 (SpendStars), PlayerCombatState.cs:190-209 (StarCostTooHigh).
    let mut c = regent_combat();
    let first = give_card(&mut c, CardKind::FallingStar, 0);
    let second = give_card(&mut c, CardKind::FallingStar, 0);
    let target = Some(enemy(&c));
    c.apply(Action::PlayCard {
        card: first,
        target,
    })
    .expect("2 of 3 stars");
    assert_eq!(stars(&c), 1);
    assert_eq!(c.state.card(first).star_cost.last_spent, 2);
    assert_eq!(
        c.apply(Action::PlayCard {
            card: second,
            target
        }),
        Err(ActionError::NotEnoughStars)
    );
}

#[test]
fn stardust_spends_every_star_and_hits_that_many_times() {
    // Stardust.cs (HasStarCostX, ResolveStarXValue), CardModel.cs:1556-1560.
    let mut c = regent_combat();
    let card = give_card(&mut c, CardKind::Stardust, 0);
    assert_eq!(c.state.star_cost(card), 3);
    let e = enemy(&c);
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard { card, target: None })
        .expect("an X star cost is always payable");
    assert_eq!(stars(&c), 0);
    assert_eq!(c.state.card(card).star_cost.last_spent, 3);
    assert_eq!(c.state.creature(e).hp, hp - 15);
}

#[test]
fn temporary_star_costs_win_last_and_expire() {
    // CardModel.cs:451-466 (CurrentStarCost), 1279-1291, 1620 (EndOfTurnCleanup).
    let mut c = regent_combat();
    let star = give_card(&mut c, CardKind::FallingStar, 0);
    let strike = give_card(&mut c, CardKind::StrikeRegent, 0);
    c.state.star_cost_mut(star).set_this_combat(1);
    c.state.star_cost_mut(star).set_this_turn(0);
    c.state.set_to_free_this_turn(strike);
    assert_eq!(c.state.star_cost(star), 0);
    // A card without a star cost set to 0 still has none.
    assert_eq!(c.state.star_cost(strike), -1);
    assert_eq!(c.state.energy_cost(strike), 0);
    c.state.end_of_turn_cleanup_of(star);
    assert_eq!(c.state.star_cost(star), 1);
}

#[test]
fn the_first_forge_generates_a_blade_and_every_forge_grows_all_blades() {
    // ForgeCmd.cs:36-80: a new blade only without a non-exhausted one; exhausted blades grow too.
    let mut c = regent_combat();
    let generated = c.state.history_count(HistoryTag::CardGenerated);
    let blades = with_engine(&mut c, |eng| eng.forge(Decimal::from(5), None));
    assert_eq!(blades.len(), 1);
    let blade = blades[0];
    assert_eq!(c.state.pile_of(blade), Some(PileType::Hand));
    assert_eq!(
        c.state.history_count(HistoryTag::CardGenerated),
        generated + 1
    );
    let again = with_engine(&mut c, |eng| eng.forge(Decimal::from(3), None));
    assert_eq!(again, [blade]);
    assert_eq!(
        c.state.history_count(HistoryTag::CardGenerated),
        generated + 1
    );
    assert_eq!(
        sovereign_blade::data(c.state.card(blade)).damage.get(),
        Decimal::from(18)
    );
    with_engine(&mut c, |eng| eng.exhaust(blade, false));
    let fresh = with_engine(&mut c, |eng| eng.forge(Decimal::from(2), None));
    assert_eq!(fresh.len(), 1);
    assert_ne!(fresh[0], blade);
    assert_eq!(
        sovereign_blade::data(c.state.card(blade)).damage.get(),
        Decimal::from(20)
    );
    assert_eq!(
        sovereign_blade::data(c.state.card(fresh[0])).damage.get(),
        Decimal::from(12)
    );
}

#[test]
fn seeking_edge_makes_the_blade_hit_every_enemy_and_parry_adds_block() {
    // SovereignBlade.cs:38 (TargetType), 122-166 (Parry block after the attack).
    let mut c = regent_combat();
    let blade = with_engine(&mut c, |eng| eng.forge(Decimal::ZERO, None))[0];
    assert_eq!(c.state.target_type(blade), TargetType::AnyEnemy);
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::SeekingEdge, Decimal::ONE, Some(p))?;
        eng.apply_power(p, PowerKind::Parry, Decimal::from(10), Some(p))
    });
    assert_eq!(c.state.target_type(blade), TargetType::AllEnemies);
    c.state.player.energy = 2;
    let block = c.state.creature(p).block;
    c.apply(Action::PlayCard {
        card: blade,
        target: None,
    })
    .expect("the blade costs 2");
    assert_eq!(c.state.creature(p).block, block + 10);
}

#[test]
fn void_form_ends_the_turn_then_frees_the_first_two_plays() {
    // VoidForm.cs (PlayerCmd.EndTurn), VoidFormPower.cs:66-103.
    let mut c = regent_combat();
    let card = give_card(&mut c, CardKind::VoidForm, 0);
    c.state.player.energy = 3;
    let round = c.state.round;
    c.apply(Action::PlayCard { card, target: None })
        .expect("3 energy");
    assert_eq!(c.state.round, round + 1);
    assert_eq!(c.state.player.phase, TurnPhase::Play);
    assert!(!c.state.player.ready_to_end_turn);
    let star = give_card(&mut c, CardKind::FallingStar, 0);
    let strike = give_card(&mut c, CardKind::StrikeRegent, 0);
    let third = give_card(&mut c, CardKind::StrikeRegent, 0);
    assert_eq!(c.state.star_cost(star), 0);
    let energy = c.state.player.energy;
    let target = Some(enemy(&c));
    for card in [star, strike] {
        c.apply(Action::PlayCard { card, target })
            .expect("free under Void Form");
    }
    assert_eq!(c.state.player.energy, energy);
    assert_eq!(c.state.energy_cost(third), 1);
}

#[test]
fn void_form_auto_played_at_turn_start_ends_that_turn_before_the_player_acts() {
    // MayhemPower.cs:15-21 plays it in the auto-pre-play phase; its PlayerCmd.EndTurn sets the
    // end-turn signal StartTurn just made (CombatManager.cs:729, 822), which the turn loop
    // awaits as soon as StartTurn returns (CombatManager.cs:617-618, 656).
    let mut c = regent_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Mayhem, Decimal::ONE, Some(p))
    });
    let void_form = give_card(&mut c, CardKind::VoidForm, 0);
    // Under the next turn's 5-card hand draw.
    c.edit(|s| {
        s.remove_from_pile(void_form);
        s.insert_into_pile(void_form, PileType::Draw, Some(5));
    });
    let round = c.state.round;
    c.apply(Action::EndTurn).expect("ends the turn");
    assert!(c.state.creature(p).has_power(PowerKind::VoidForm));
    assert_eq!(c.state.round, round + 2);
    assert_eq!(c.state.player.phase, TurnPhase::Play);
    assert!(!c.state.player.ready_to_end_turn);
}

#[test]
fn royalties_adds_its_gold_as_an_extra_reward_on_a_win() {
    // RoyaltiesPower.cs:14, CombatRoom.cs:185.
    let mut c = regent_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Royalties, Decimal::from(30), Some(p))
    });
    for &e in &c.state.enemies.clone() {
        c.state.creature_mut(e).hp = 1;
    }
    let card = give_card(&mut c, CardKind::Stardust, 0);
    c.apply(Action::PlayCard { card, target: None })
        .expect("3 stars");
    assert_eq!(c.status(), Status::Won);
    assert_eq!(c.state.extra_rewards, [ExtraReward::Gold(30)]);
}
