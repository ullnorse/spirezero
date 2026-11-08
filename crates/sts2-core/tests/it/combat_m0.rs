//! Combat milestone M0 through the public API: Necrobinder vs `FuzzyWurmCrawlerWeak`.
//! Expected values are derived by hand from the specs and the decomp (paths under
//! `ref/decomp/MegaCrit.Sts2.Core.`).

use sts2_core::combat::{CardId, Event, Side};
use sts2_core::content::cards::CardKind;
use sts2_core::rng::{RunRngSet, RunRngType};
use sts2_core::shuffle::{stable_shuffle_by, unstable_shuffle};
use sts2_core::{Action, Combat, CombatConfig, Status};

const SEED: &str = "94LYYA8YDC";

/// The starter deck in deck order (`Models.Characters/Necrobinder.cs:48-60`).
const DECK: [CardKind; 10] = [
    CardKind::StrikeNecrobinder,
    CardKind::StrikeNecrobinder,
    CardKind::StrikeNecrobinder,
    CardKind::StrikeNecrobinder,
    CardKind::DefendNecrobinder,
    CardKind::DefendNecrobinder,
    CardKind::DefendNecrobinder,
    CardKind::DefendNecrobinder,
    CardKind::Bodyguard,
    CardKind::Unleash,
];

fn hand_kinds(c: &Combat) -> Vec<CardKind> {
    let s = c.state();
    s.player.hand.iter().map(|&id| s.card(id).kind).collect()
}

#[test]
fn setup_rolls_monster_hp_on_niche_and_shuffles_the_deck_on_shuffle() {
    // Combat/CombatState.cs:239-241, Entities.Creatures/Creature.cs:372-384: HP is NextItem
    // over 55..=57 on Rng.Niche. Entities.Players/Player.cs:806, Entities.Cards/CardPile.cs:69:
    // deck order, then UnstableShuffle on Rng.Shuffle; draw takes index 0.
    let c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
    let mut rngs = RunRngSet::new(SEED);
    let hp = *rngs[RunRngType::Niche]
        .next_item(&[55, 56, 57])
        .expect("non-empty");
    let enemy = c.state().creature(c.state().enemies[0]);
    assert_eq!((enemy.hp, enemy.max_hp), (hp, hp));

    let mut order: Vec<usize> = (0..DECK.len()).collect();
    unstable_shuffle(&mut order, &mut rngs[RunRngType::Shuffle]);
    let expected: Vec<CardKind> = order[..5].iter().map(|&i| DECK[i]).collect();
    assert_eq!(hand_kinds(&c), expected);
    // Regression pin for this seed.
    assert_eq!(
        hand_kinds(&c),
        [
            CardKind::DefendNecrobinder,
            CardKind::StrikeNecrobinder,
            CardKind::Bodyguard,
            CardKind::StrikeNecrobinder,
            CardKind::StrikeNecrobinder,
        ]
    );
    let draw: Vec<CardId> = c.state().player.draw.to_vec();
    let expected_draw: Vec<CardId> = order[5..]
        .iter()
        .map(|&i| CardId(u32::try_from(i).expect("small")))
        .collect();
    assert_eq!(draw, expected_draw);
    assert_eq!(
        rngs[RunRngType::Shuffle],
        c.state().rng[RunRngType::Shuffle]
    );
}

#[test]
fn reshuffle_sorts_by_model_id_then_stable_shuffles() {
    // Commands/CardPileCmd.cs:1076, Models/CardModel.cs:2272, Models/ModelId.cs:42: when the
    // draw pile runs out, discard is sorted by (entry, upgrade) and StableShuffled on
    // Rng.Shuffle. Ending every turn flushes 5 + 5 cards, so turn 3 reshuffles all 10.
    let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
    c.apply(Action::EndTurn).expect("legal");
    let discard = c.state().player.discard.to_vec();
    let hand = c.state().player.hand.to_vec();
    let mut rng = c.state().rng[RunRngType::Shuffle].clone();
    c.apply(Action::EndTurn).expect("legal");

    let mut list = discard;
    list.extend(hand);
    let cards = c.state().cards.clone();
    stable_shuffle_by(&mut list, &mut rng, |a, b| {
        let (a, b) = (&cards[a.0 as usize], &cards[b.0 as usize]);
        a.kind.entry().cmp(b.kind.entry())
    });
    assert_eq!(c.state().player.hand, list[..5]);
    assert_eq!(c.state().player.draw, list[5..]);
    assert!(c.events().contains(&Event::Shuffled));
}

/// Total damage of each enemy turn, `None` when the monster did not attack.
fn enemy_turn_damage(events: &[Event], monster: sts2_core::combat::CreatureId) -> Vec<Option<i32>> {
    let mut turns: Vec<Option<i32>> = Vec::new();
    let mut in_enemy_turn = false;
    for event in events {
        match event {
            Event::TurnStarted { side, .. } => {
                in_enemy_turn = *side == Side::Enemy;
                if in_enemy_turn {
                    turns.push(None);
                }
            }
            Event::Damage {
                dealer: Some(d),
                blocked,
                hp_lost,
                ..
            } if in_enemy_turn && *d == monster => {
                let total = turns.last_mut().expect("in an enemy turn");
                *total = Some(total.unwrap_or(0) + blocked + hp_lost);
            }
            _ => {}
        }
    }
    turns
}

#[test]
fn passive_player_takes_the_acid_goop_cycle_and_loses() {
    // Models.Monsters/FuzzyWurmCrawler.cs:48-88: FIRST_ACID_GOOP (4), INHALE (+7 Strength),
    // ACID_GOOP, repeat; Strength adds in ModifyDamageAdditive. The move is rolled at each
    // player turn start, so the cycle is 4, -, 11, 11, -, 18, 18, -, 25.
    // Bound Phylactery revives/grows Osty by 1 each turn, and Osty soaks unblocked damage.
    // The loss is processed at the CheckWinCondition after the enemy's move
    // (Combat/CombatManager.cs:1426), so no further player turn starts.
    let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
    let monster = c.state().enemies[0];
    while c.status() == Status::PlayerTurn {
        c.apply(Action::EndTurn).expect("legal");
    }
    assert_eq!(c.status(), Status::Lost);
    let damage = enemy_turn_damage(c.events(), monster);
    let cycle = [
        Some(4),
        None,
        Some(11),
        Some(11),
        None,
        Some(18),
        Some(18),
        None,
    ];
    assert_eq!(damage[..8], cycle);

    // Osty has 1 HP on turn 1 and is summoned for 1 each later turn (grown when alive,
    // revived at 1/1 when dead), soaking its HP from each hit. Osty HP at each attack:
    // 1, 2, 1, 2, 1, 2, so the player loses 3, 9, 10, 16, 17 (11 HP left); the 25-damage
    // hit on turn 9 kills Osty (2) and then the player (11), which ends the record.
    assert_eq!(damage.len(), 9);
    assert_eq!(damage[8], Some(2 + 11));
    let player = c.state().creature(c.state().player.creature);
    assert_eq!(player.hp, 0);
    assert_eq!(c.events().last(), Some(&Event::Lost));
    let last_turn = c.events().iter().rev().find_map(|e| match e {
        Event::TurnStarted { side, .. } => Some(*side),
        _ => None,
    });
    assert_eq!(last_turn, Some(Side::Enemy));
    assert_eq!(c.legal_actions(), []);
}

#[test]
fn defend_block_is_spent_before_osty() {
    // CreatureCmd.cs:285-289: block absorbs first; only the unblocked rest reaches Osty.
    // Block is not cleared at the start of the player's first turn (Creature.cs:686-733).
    let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
    let defend = c
        .state()
        .player
        .hand
        .iter()
        .copied()
        .find(|&id| c.state().card(id).kind == CardKind::DefendNecrobinder)
        .expect("seed deals a Defend");
    c.apply(Action::PlayCard {
        card: defend,
        target: None,
    })
    .expect("legal");
    let player = c.state().player.creature;
    assert_eq!(c.state().creature(player).block, 5);
    c.apply(Action::EndTurn).expect("legal");
    // 4 damage into 5 block; the block then clears at the start of turn 2.
    let osty = c.state().osty().expect("summoned");
    assert_eq!(c.state().creature(player).hp, 66);
    assert_eq!(c.state().creature(player).block, 0);
    assert_eq!(
        c.state().creature(osty).hp,
        2,
        "1 + 1 from Bound Phylactery"
    );
}

#[test]
fn bound_phylactery_summons_before_combat_and_each_later_turn() {
    // Models.Relics/BoundPhylactery.cs:22-38.
    let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
    let osty = c.state().osty().expect("summoned before combat");
    assert_eq!(c.state().creature(osty).max_hp, 1);
    let body = c
        .state()
        .player
        .hand
        .iter()
        .copied()
        .find(|&id| c.state().card(id).kind == CardKind::Bodyguard)
        .expect("seed deals Bodyguard");
    c.apply(Action::PlayCard {
        card: body,
        target: None,
    })
    .expect("legal");
    assert_eq!(c.state().creature(osty).hp, 6);
    // Turn 1 attack: 4 into Osty (6 -> 2), then +1 at turn 2.
    c.apply(Action::EndTurn).expect("legal");
    assert_eq!(
        (c.state().creature(osty).hp, c.state().creature(osty).max_hp),
        (3, 7)
    );
}

#[test]
fn trivial_policy_plays_a_full_fight() {
    let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
    let mut actions = 0;
    while matches!(c.status(), Status::PlayerTurn | Status::Decision) {
        let legal = c.legal_actions();
        let action = legal
            .iter()
            .find(|a| !matches!(a, Action::EndTurn))
            .unwrap_or(&legal[0])
            .clone();
        c.apply(action).expect("legal action");
        actions += 1;
        assert!(actions < 500, "fight does not terminate");
    }
    assert_eq!(c.status(), Status::Won);
    assert_eq!(c.events().last(), Some(&Event::Won));
}

#[test]
fn same_seed_and_actions_give_the_same_combat() {
    let play = || {
        let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0).with_events());
        for _ in 0..3 {
            let first = c.legal_actions()[0].clone();
            c.apply(first).expect("legal");
            c.apply(Action::EndTurn).expect("legal");
        }
        c
    };
    assert_eq!(play(), play());
}
