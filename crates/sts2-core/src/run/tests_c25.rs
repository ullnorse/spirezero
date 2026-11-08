//! Batch C25: `Claws`' up-to selection and `CardCmd.Transform`'s order, `MeatCleaver`'s Cook
//! option, `PreservedFog`'s `Folly` after its removal screen, and `LordsParasol`'s free shop.

use super::*;

/// `T6S0` after Neow (Phial Holster), in test mode.
fn after_neow() -> Run {
    let mut run = Run::new(
        Character::Necrobinder,
        "T6S0",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("T6S0 starts")
    .in_test_mode();
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    run
}

/// `RelicCmd.Obtain(id)`; a prompt it opens becomes the phase.
fn obtain(run: &mut Run, id: &'static str) {
    if let Some(phase) = run.obtain_relic(id).expect("ported") {
        run.phase = phase;
    }
}

fn select(run: &Run) -> &CardSelect {
    match run.pending_decision() {
        RunDecision::CardSelect(select) => select,
        d => panic!("not a card selection: {d:?}"),
    }
}

#[test]
fn claws_may_transform_no_card() {
    let mut run = after_neow();
    let deck = run.state.deck.clone();
    obtain(&mut run, "CLAWS");
    let s = select(&run);
    assert_eq!((s.min(), s.count), (0, 6));
    assert!(run.legal_actions().contains(&RunAction::ConfirmSelect));
    run.apply(RunAction::ConfirmSelect).expect("up to 6");
    assert_eq!(run.state.deck, deck);
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

/// Picks in the order `4, 1, 5, 2, 6, 7` (an upgraded card at 4): `CardCmd.Transform` notes
/// the indices `4, 1, 3, 1, 2, 2` as the originals leave, so the upgraded `Maul` joins last
/// (seen on the referee, `RL2S2` Ironclad).
#[test]
fn claws_adds_the_mauls_by_the_indices_at_removal() {
    let mut run = after_neow();
    run.state.deck[4].upgrade();
    obtain(&mut run, "CLAWS");
    for i in [4, 1, 5, 2, 6, 7] {
        run.apply(RunAction::SelectCard(i)).expect("a pick");
    }
    let mauls: Vec<u8> = run
        .state
        .deck
        .iter()
        .filter(|c| c.id == "MAUL")
        .map(|c| c.upgrade_level)
        .collect();
    assert_eq!(mauls, [0, 0, 0, 0, 0, 1]);
}

fn rest_site(run: &mut Run) -> Vec<RestOption> {
    run.state.room = Room::RestSite;
    let options = run.rest_options();
    run.phase = Phase::RestSite(RestSite {
        options: options.clone(),
        used: false,
        pending: None,
    });
    options
}

#[test]
fn meat_cleaver_cooks_two_cards_into_max_hp() {
    let mut run = after_neow();
    obtain(&mut run, "MEAT_CLEAVER");
    let options = rest_site(&mut run);
    assert_eq!(
        options[2],
        RestOption {
            kind: RestOptionKind::Cook,
            enabled: true
        }
    );
    let (deck, max_hp, hp) = (run.state.deck.len(), run.state.max_hp, run.state.hp);
    run.apply(RunAction::RestOption(2)).expect("enabled");
    let s = select(&run);
    assert_eq!((s.min(), s.count), (2, 2));
    // Cancelable: back at the rest site with Cook still there.
    run.apply(RunAction::CancelSelect).expect("cancelable");
    assert!(matches!(run.pending_decision(), RunDecision::RestSite(o) if o.len() == 3));
    run.apply(RunAction::RestOption(2)).expect("enabled");
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    run.apply(RunAction::SelectCard(1)).expect("a pick");
    assert_eq!(run.state.deck.len(), deck - 2);
    assert_eq!((run.state.max_hp, run.state.hp), (max_hp + 5, hp + 5));
}

#[test]
fn meat_cleaver_cook_needs_two_removable_cards() {
    let mut run = after_neow();
    obtain(&mut run, "MEAT_CLEAVER");
    run.state.deck.truncate(1);
    let options = rest_site(&mut run);
    assert!(!options[2].enabled);
}

#[test]
fn preserved_fog_adds_folly_after_the_removal_screen() {
    let mut run = after_neow();
    let deck = run.state.deck.len();
    obtain(&mut run, "PRESERVED_FOG");
    assert!(!run.state.deck.iter().any(|c| c.id == "FOLLY"));
    for i in 0..3 {
        run.apply(RunAction::SelectCard(i)).expect("a pick");
    }
    assert_eq!(run.state.deck.len(), deck - 3 + 1);
    assert_eq!(run.state.deck.last().map(|c| c.id), Some("FOLLY"));
}

#[test]
fn preserved_fog_without_a_screen_still_adds_folly_last() {
    let mut run = after_neow();
    run.state.deck.truncate(2);
    obtain(&mut run, "PRESERVED_FOG");
    let ids: Vec<&str> = run.state.deck.iter().map(|c| c.id).collect();
    assert_eq!(ids, ["FOLLY"]);
}

#[test]
fn lords_parasol_buys_everything_for_free_then_removes_a_card() {
    let mut run = after_neow();
    obtain(&mut run, "LORDS_PARASOL");
    run.state.gold = 0;
    let (deck, relics) = (run.state.deck.len(), run.state.relics.len());
    run.state.room = Room::Shop;
    let mut shop = Shop::generate(&mut run.state);
    let held = run.state.potions.iter().flatten().count();
    let potions = shop.potions.iter().filter_map(|p| p.id).count();
    hooks::after_merchant_room_entered(&run.state, &mut shop);
    run.phase = Phase::Shop(Box::new(shop));
    run.continue_free_purchases().expect("ported");
    // A bought relic's own prompt comes first (here `T6S0`'s shop has an enchanting relic).
    while !matches!(select(&run).purpose, SelectPurpose::ShopRemoval { .. }) {
        let next = (0..select(&run).options.len()).find(|i| !select(&run).picked.contains(i));
        run.apply(RunAction::SelectCard(next.expect("an option")))
            .expect("a pick");
    }
    // The removal is next: free and not cancelable.
    let s = select(&run);
    assert_eq!(
        s.purpose,
        SelectPurpose::ShopRemoval {
            cost: 0,
            cancelable: false
        }
    );
    assert!(!run.legal_actions().contains(&RunAction::CancelSelect));
    assert_eq!(run.state.deck.len(), deck + 7);
    assert_eq!(run.state.relics.len(), relics + 3);
    // The belt refuses what does not fit; those potions stay.
    assert_eq!(
        run.state.potions.iter().flatten().count(),
        (held + potions).min(run.state.potions.len())
    );
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    let RunDecision::Shop(shop) = run.pending_decision() else {
        panic!("back in the shop");
    };
    assert!(shop.removal_used && shop.free_purchases.is_empty());
    assert!(shop.cards.iter().all(|c| c.card.is_none()));
    assert!(shop.relics.iter().all(|r| r.id.is_none()));
    assert_eq!(run.state.deck.len(), deck + 6);
    assert_eq!(run.state.gold, 0);
    assert_eq!(run.state.card_shop_removals_used, 1);
}
