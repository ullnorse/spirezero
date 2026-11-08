//! The Overgrowth weak encounters through the public API. Expected values come from the decomp
//! (paths under `ref/decomp/MegaCrit.Sts2.Core.`) or, where marked, from referee runs.

use sts2_core::combat::{Encounter, Event, PileType};
use sts2_core::content::cards::CardKind;
use sts2_core::content::monsters::{
    LeafSlimeS, MonsterKind, Nibbit, ShrinkerBeetle, TwigSlimeM, TwigSlimeS,
};
use sts2_core::content::powers::PowerKind;
use sts2_core::{Action, Combat, CombatConfig, Status};

fn combat(seed: &str, encounter: Encounter) -> Combat {
    Combat::new(
        CombatConfig::necrobinder(seed, 0)
            .with_encounter(encounter)
            .with_events(),
    )
}

/// `(kind, hp, next move)` of every enemy.
fn enemies(c: &Combat) -> Vec<(MonsterKind, i32, &'static str)> {
    let s = c.state();
    s.enemies
        .iter()
        .map(|&e| {
            let creature = s.creature(e);
            let monster = creature.monster.as_ref().expect("enemy is a monster");
            (monster.kind, creature.hp, monster.next_move_name())
        })
        .collect()
}

#[test]
fn slimes_weak_matches_the_referee_composition() {
    // Referee, seed D1S0: LEAF_SLIME_S 14 GOOP_MOVE, TWIG_SLIME_M 26 STICKY_SHOT_MOVE,
    // TWIG_SLIME_S 7 TACKLE_MOVE.
    let c = combat("D1S0", Encounter::SlimesWeak);
    assert_eq!(
        enemies(&c),
        [
            (MonsterKind::LeafSlimeS(LeafSlimeS), 14, "GOOP_MOVE"),
            (MonsterKind::TwigSlimeM(TwigSlimeM), 26, "STICKY_SHOT_MOVE"),
            (MonsterKind::TwigSlimeS(TwigSlimeS), 7, "TACKLE_MOVE"),
        ]
    );
}

#[test]
fn slimes_add_slimed_to_the_discard_pile() {
    // Models.Monsters/LeafSlimeS.cs (Goop: 1 Slimed), TwigSlimeM.cs (Sticky Shot: 1 Slimed).
    let mut c = combat("D1S0", Encounter::SlimesWeak);
    assert_eq!(c.apply(Action::EndTurn), Ok(Status::PlayerTurn));
    let s = c.state();
    let slimed = |pile: PileType| {
        s.player
            .pile(pile)
            .iter()
            .filter(|&&id| s.card(id).kind == CardKind::Slimed)
            .count()
    };
    let total: usize = PileType::LISTENER_ORDER.into_iter().map(slimed).sum();
    assert_eq!(total, 2);
}

#[test]
fn nibbit_alone_opens_with_butt() {
    // Models.Monsters/Nibbit.cs:62-87: alone, the conditional opener picks BUTT_MOVE.
    let c = combat("D1S0", Encounter::NibbitsWeak);
    let [(kind, _, next)] = enemies(&c)[..] else {
        panic!("one Nibbit");
    };
    assert_eq!(
        kind,
        MonsterKind::Nibbit(Nibbit {
            is_front: false,
            is_alone: true
        })
    );
    assert_eq!(next, "BUTT_MOVE");
}

#[test]
fn shrink_cuts_the_players_attack_damage_by_30_percent() {
    // Models.Powers/ShrinkPower.cs: x0.7 on the owner's powered attacks; Strike 6 -> 4.2 -> 4.
    let mut c = combat("D1S0", Encounter::ShrinkerBeetleWeak);
    assert_eq!(
        enemies(&c)[0].0,
        MonsterKind::ShrinkerBeetle(ShrinkerBeetle)
    );
    assert_eq!(c.apply(Action::EndTurn), Ok(Status::PlayerTurn));
    let s = c.state();
    let player = s.creature(s.player.creature);
    assert_eq!(player.power_amount(PowerKind::Shrink), -1);
    let strike = *s
        .player
        .hand
        .iter()
        .find(|&&id| s.card(id).kind == CardKind::StrikeNecrobinder)
        .expect("a Strike in hand on D1S0 turn 2");
    let beetle = s.enemies[0];
    let before = c.events().len();
    c.apply(Action::PlayCard {
        card: strike,
        target: Some(beetle),
    })
    .expect("Strike is playable");
    let lost: i32 = c.events()[before..]
        .iter()
        .filter_map(|e| match e {
            Event::Damage {
                receiver, hp_lost, ..
            } if *receiver == beetle => Some(*hp_lost),
            _ => None,
        })
        .sum();
    assert_eq!(lost, 4);
}
