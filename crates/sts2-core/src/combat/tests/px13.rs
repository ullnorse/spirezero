//! The turn-1 draw pile's `ShouldStartAtBottomOfDrawPile` cards and the state-reading
//! `ShouldOwnerDeathTriggerFatal` (PX13; paths under `ref/decomp/MegaCrit.Sts2.Core.`).

use super::*;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::content::encounters::Encounter;

fn imbued(kind: CardKind) -> DeckCard {
    DeckCard {
        enchantment: Some(Enchantment::new(EnchantmentKind::TestImbued, 1)),
        ..DeckCard::new(kind)
    }
}

#[test]
fn bottom_of_draw_pile_cards_skip_the_innate_move_and_the_opening_hand() {
    // CombatManager.cs:905-919: the enchanted cards move to the bottom first, and the Innate
    // cards except those move to the top; the hand draw is max(5, that Innate count).
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.deck.extend([
        imbued(CardKind::Backstab),
        imbued(CardKind::StrikeNecrobinder),
        DeckCard::new(CardKind::Backstab),
    ]);
    let n = config.deck.len();
    let c = Combat::new(config);
    let deck_card = |id: CardId| c.state.card(id).deck_version();
    let draw = &c.state.player.draw;
    let bottom: Vec<Option<usize>> = draw[draw.len() - 2..]
        .iter()
        .map(|&id| deck_card(id))
        .collect();
    assert!(bottom == [Some(n - 3), Some(n - 2)] || bottom == [Some(n - 2), Some(n - 3)]);
    assert_eq!(c.state.player.hand.len(), 5);
    assert!(
        c.state
            .player
            .hand
            .iter()
            .any(|&id| deck_card(id) == Some(n - 1))
    );
}

#[test]
fn reattach_allows_fatal_only_for_the_last_segment() {
    // ReattachPower.cs:106-109: ShouldOwnerDeathTriggerFatal is AreAllOtherSegmentsDead().
    let mut c = Combat::new(
        CombatConfig::necrobinder(SEED, 0).with_encounter(Encounter::DecimillipedeElite),
    );
    let segments = c.state.enemies.clone();
    assert!(segments.len() > 1);
    let last = segments[segments.len() - 1];
    with_engine(&mut c, |eng| {
        assert!(!eng.target_allows_fatal(last));
        Ok(())
    });
    for &s in &segments[..segments.len() - 1] {
        c.state.creature_mut(s).hp = 0;
    }
    with_engine(&mut c, |eng| {
        assert!(eng.target_allows_fatal(last));
        Ok(())
    });
}
