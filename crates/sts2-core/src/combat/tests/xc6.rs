//! Run-deck theft (XC6): `SwipePower.Steal` takes a card out of the combat and the run deck;
//! the thief's death returns it as a `SpecialCardReward`, its escape keeps it.

use super::*;
use crate::content::powers::test_swipe;

/// Steals the first draw-pile card cloned from the deck; returns it and its deck index.
fn steal_from_draw(c: &mut Combat, thief: CreatureId) -> (CardId, usize) {
    let card = c
        .state
        .player
        .draw
        .iter()
        .copied()
        .find(|&id| c.state.card(id).deck_version().is_some())
        .expect("a deck card in the draw pile");
    let deck_index = c.state.card(card).deck_version().expect("a deck card");
    with_engine(c, |eng| test_swipe::steal(eng, thief, card));
    (card, deck_index)
}

#[test]
fn a_stolen_card_leaves_the_combat_and_the_deck() {
    // ThievingHopper.cs:223-244, SwipePower.cs:69-78, CardPileCmd.cs:53-89,110-206.
    let mut c = new_combat();
    let thief = enemy(&c);
    let (card, deck_index) = steal_from_draw(&mut c, thief);
    assert_eq!(c.state.pile_of(card), None);
    assert!(!c.state.player.draw.contains(&card));
    assert_eq!(c.state.removed_from_deck, [deck_index]);
    // The fight's view of the deck is unchanged; the run removes the card after the fight.
    assert_eq!(
        c.state.deck.len(),
        CombatConfig::necrobinder(SEED, 0).deck.len()
    );
    assert!(c.state.creature(thief).has_power(PowerKind::TestSwipe));
}

#[test]
fn killing_the_thief_returns_the_card_as_a_special_card_reward() {
    // SwipePower.cs:50-67.
    let mut c = new_combat();
    let thief = enemy(&c);
    let (_, deck_index) = steal_from_draw(&mut c, thief);
    with_engine(&mut c, |eng| eng.kill(&[thief]));
    assert_eq!(
        c.state.extra_rewards,
        [ExtraReward::SpecialCard(deck_index)]
    );
    // Still out of the deck until the reward is claimed.
    assert_eq!(c.state.removed_from_deck, [deck_index]);
}

#[test]
fn an_escaped_thief_keeps_the_card() {
    // CreatureCmd.Escape drops the powers without `BeforeDeath`.
    let mut c = new_combat();
    let thief = enemy(&c);
    let (_, deck_index) = steal_from_draw(&mut c, thief);
    with_engine(&mut c, |eng| {
        eng.escape(thief);
        Ok(())
    });
    assert_eq!(c.state.extra_rewards, []);
    assert_eq!(c.state.removed_from_deck, [deck_index]);
}
