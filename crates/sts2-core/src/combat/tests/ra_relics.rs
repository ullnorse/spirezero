//! Counter relics of batch RA: the saved counters come in with the relic's data and the effects
//! fire at the right count (paths under `ref/decomp/MegaCrit.Sts2.Core.`, `Models.Relics/`).

use super::*;
use crate::content::relics::{
    OwnedRelic, RelicData, beating_remnant, happy_flower, joss_paper, lizard_tail, pen_nib,
    pendulum, tuning_fork,
};

fn with_data(relic: RelicKind, data: RelicData) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.relics.push(OwnedRelic {
        kind: relic,
        data,
        melted: false,
    });
    Combat::new(config)
}

fn play(c: &mut Combat, card: CardId, target: Option<CreatureId>) {
    c.apply(Action::PlayCard { card, target }).expect("legal");
}

#[test]
fn pen_nib_doubles_the_tenth_attack() {
    // PenNib.cs: `AttacksPlayed` is saved; the attack that brings it to 10 deals double damage.
    let mut c = with_data(
        RelicKind::PenNib,
        RelicData::PenNib(pen_nib::Data {
            attacks_played: 9,
            attack_to_double: None,
        }),
    );
    let target = enemy(&c);
    let hp = c.state.creature(target).hp;
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    play(&mut c, strike, Some(target));
    assert_eq!(c.state.creature(target).hp, hp - 12, "6 doubled");
    let RelicData::PenNib(data) = c.state.relic(RelicId(1)).data else {
        panic!("not a PenNib");
    };
    assert_eq!(data.attacks_played, 0);
    assert_eq!(data.attack_to_double, None);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    play(&mut c, strike, Some(target));
    assert_eq!(
        c.state.creature(target).hp,
        hp - 18,
        "the next one is plain"
    );
}

#[test]
fn happy_flower_gives_energy_on_every_third_turn() {
    // HappyFlower.cs: `TurnsSeen = (TurnsSeen + 1) % 3` after each side turn start; 0 gains 1.
    let c = with_data(
        RelicKind::HappyFlower,
        RelicData::HappyFlower(happy_flower::Data { turns_seen: 2 }),
    );
    assert_eq!(c.state.player.energy, 4);
    let RelicData::HappyFlower(data) = c.state.relic(RelicId(1)).data else {
        panic!("not a HappyFlower");
    };
    assert_eq!(data.turns_seen, 0);
}

#[test]
fn pendulum_draws_an_extra_card_on_every_third_turn() {
    // Pendulum.cs: `BeforeHandDraw` advances `TurnsSeen`; `ModifyHandDraw` adds 1 at 0.
    let plain = Combat::new(CombatConfig::necrobinder(SEED, 0));
    let c = with_data(
        RelicKind::Pendulum,
        RelicData::Pendulum(pendulum::Data { turns_seen: 2 }),
    );
    assert_eq!(c.state.player.hand.len(), plain.state.player.hand.len() + 1);
}

#[test]
fn tuning_fork_gives_block_on_the_tenth_skill() {
    // TuningFork.cs: `SkillsPlayed` is saved; at 10 the owner gains 7 Block and the count wraps.
    let mut c = with_data(
        RelicKind::TuningFork,
        RelicData::TuningFork(tuning_fork::Data { skills_played: 9 }),
    );
    let p = player(&c);
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    play(&mut c, defend, None);
    assert_eq!(c.state.creature(p).block, 5 + 7);
    let RelicData::TuningFork(data) = c.state.relic(RelicId(1)).data else {
        panic!("not a TuningFork");
    };
    assert_eq!(data.skills_played, 0);
}

#[test]
fn joss_paper_draws_when_the_count_reaches_five() {
    // JossPaper.cs: exhausting a card counts; every 5 draws one card and keeps the remainder.
    let mut c = with_data(
        RelicKind::JossPaper,
        RelicData::JossPaper(joss_paper::Data {
            cards_exhausted: 4,
            ethereal_count: 0,
        }),
    );
    let hand = c.state.player.hand.len();
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    with_engine(&mut c, |eng| eng.exhaust(defend, false));
    assert_eq!(
        c.state.player.hand.len(),
        hand + 1,
        "one drawn for one exhausted"
    );
    let RelicData::JossPaper(data) = c.state.relic(RelicId(1)).data else {
        panic!("not a JossPaper");
    };
    assert_eq!(data.cards_exhausted, 0);
}

#[test]
fn lizard_tail_saves_the_owner_once() {
    // LizardTail.cs: `ShouldDieLate` is false until `WasUsed`; the save heals 50% of max HP.
    let mut c = with_data(
        RelicKind::LizardTail,
        RelicData::LizardTail(lizard_tail::Data::default()),
    );
    let p = player(&c);
    let max_hp = c.state.creature(p).max_hp;
    for round in 0..2 {
        with_engine(&mut c, |eng| {
            eng.damage(
                &[p],
                Decimal::from(500),
                ValueProps::UNBLOCKABLE,
                None,
                None,
            )
        });
        if round == 0 {
            assert_eq!(c.state.creature(p).hp, max_hp / 2);
        } else {
            assert!(c.state.creature(p).is_dead(), "only once");
        }
    }
}

#[test]
fn beating_remnant_caps_the_hp_lost_per_turn() {
    // BeatingRemnant.cs: `min(amount, 20 - DamageReceivedThisTurn)`.
    let mut c = with_data(
        RelicKind::BeatingRemnant,
        RelicData::BeatingRemnant(beating_remnant::Data::default()),
    );
    let p = player(&c);
    let hp = c.state.creature(p).hp;
    let dealer = Some(enemy(&c));
    for expected in [hp - 20, hp - 20] {
        with_engine(&mut c, |eng| {
            eng.damage(&[p], Decimal::from(30), ValueProps::MOVE, dealer, None)
        });
        assert_eq!(c.state.creature(p).hp, expected);
    }
}

#[test]
fn a_melted_relic_does_not_listen() {
    // `CombatState.IterateHookListeners` skips `IsMelted` relics (`ToyBox`'s wax ones): Anchor's
    // 10 block at the start of combat comes only from an unmelted one.
    let block = |melted| {
        let mut config = CombatConfig::necrobinder(SEED, 0);
        config.relics.push(OwnedRelic {
            kind: RelicKind::Anchor,
            data: RelicData::None,
            melted,
        });
        let c = Combat::new(config);
        c.state.creature(c.state.player.creature).block
    };
    assert_eq!(block(false), 10);
    assert_eq!(block(true), 0);
}
