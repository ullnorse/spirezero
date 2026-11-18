//! `Combat::determinize` (RLN28): hides the draw order and the fight's random streams, and
//! nothing the player sees.

use super::*;

fn sorted(pile: &state::Pile) -> Vec<CardId> {
    let mut cards = pile.to_vec();
    cards.sort_unstable();
    cards
}

/// The combat with every monster's own stream set to `like`'s, to compare the rest.
fn without_monster_streams(c: &Combat, like: &Combat) -> Vec<Creature> {
    c.state
        .creatures
        .iter()
        .zip(&like.state.creatures)
        .map(|(a, b)| {
            let mut a = a.clone();
            if let (Some(m), Some(n)) = (a.monster.as_mut(), b.monster.as_ref()) {
                m.rng = n.rng.clone();
            }
            a
        })
        .collect()
}

#[test]
fn determinize_is_deterministic_in_its_seed() {
    let c = new_combat();
    let (mut a, mut b, mut other) = (c.clone(), c.clone(), c);
    a.determinize(1);
    b.determinize(1);
    other.determinize(2);
    assert_eq!(a.state, b.state);
    assert_ne!(a.state.rng, other.state.rng);
}

#[test]
fn determinize_keeps_what_the_player_sees() {
    let c = new_combat();
    let mut d = c.clone();
    d.determinize(7);
    let (p, q) = (&c.state.player, &d.state.player);
    assert_eq!(p.hand, q.hand);
    assert_eq!(p.discard, q.discard);
    assert_eq!(p.exhaust, q.exhaust);
    assert_eq!(sorted(&p.draw), sorted(&q.draw));
    assert_eq!(p.energy, q.energy);
    assert_eq!(without_monster_streams(&d, &c), c.state.creatures);
    assert_eq!(d.state.encounter, c.state.encounter);
    // Out-of-combat streams stay; the fight's are reseeded, and so is each monster's.
    for kept in [
        RunRngType::UpFront,
        RunRngType::UnknownMapPoint,
        RunRngType::TreasureRoomRelics,
    ] {
        assert_eq!(d.state.rng[kept], c.state.rng[kept]);
    }
    for hidden in [
        RunRngType::Shuffle,
        RunRngType::MonsterAi,
        RunRngType::Niche,
    ] {
        assert_ne!(d.state.rng[hidden], c.state.rng[hidden]);
    }
    let monster = |c: &Combat| {
        c.state
            .creature(enemy(c))
            .monster
            .as_ref()
            .map(|m| m.rng.clone())
    };
    assert_ne!(monster(&d), monster(&c));
}

#[test]
fn determinize_reshuffles_the_draw_pile() {
    let c = new_combat();
    assert!(c.state.player.draw.len() >= 3);
    let orders: Vec<Vec<CardId>> = (0..8)
        .map(|seed| {
            let mut d = c.clone();
            d.determinize(seed);
            d.state.player.draw.to_vec()
        })
        .collect();
    assert!(orders.iter().any(|o| *o != c.state.player.draw.to_vec()));
    assert!(orders.iter().any(|o| *o != orders[0]));
}
