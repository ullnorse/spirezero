//! One test per modifier: the effect it has on a run.

use super::*;
use crate::combat::Combat;
use crate::content::cards::{CardKind, CardType};
use crate::content::powers::PowerKind;
use crate::run::map::ActMap;
use crate::run::rewards::{CardCreation, CardPool, ExtraPools};
use crate::run::{
    Act1, Phase, RestOption, RestOptionKind, Run, RunAction, RunCard, RunError, UnlockState,
};

fn run_with(modifiers: &[Modifier]) -> Run {
    Run::new_with_modifiers(
        Character::Necrobinder,
        "GD39",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
        modifiers,
    )
    .expect("the run starts")
}

/// Takes the (only) option of the current Neow page.
fn take_neow(run: &mut Run) {
    assert!(matches!(run.phase, Phase::Ancient(_)));
    run.apply(RunAction::AncientOption(0)).expect("the option");
}

fn starter_len() -> usize {
    Character::Necrobinder.starting_deck().len()
}

#[test]
fn flight_allows_free_travel() {
    assert!(!run_with(&[]).state().modifiers.contains(&Modifier::Flight));
    assert!(allows_free_travel(run_with(&[Modifier::Flight]).state()));
    assert!(!allows_free_travel(run_with(&[Modifier::Hoarder]).state()));
}

#[test]
fn midas_doubles_gold_and_removes_smith() {
    let mut rewards = [Reward::Gold(10), Reward::Gold(7)];
    let mut added = [PendingReward::Ready(Reward::Gold(5))];
    midas::double_gold(&mut rewards, &mut added);
    assert_eq!(rewards, [Reward::Gold(20), Reward::Gold(14)]);
    assert_eq!(added[0], PendingReward::Ready(Reward::Gold(10)));
    let run = run_with(&[Modifier::Midas]);
    let mut options = vec![
        RestOption {
            kind: RestOptionKind::Heal,
            enabled: true,
        },
        RestOption {
            kind: RestOptionKind::Smith,
            enabled: true,
        },
    ];
    modify_rest_site_options(run.state(), &mut options);
    assert!(options.iter().all(|o| o.kind != RestOptionKind::Smith));
    assert_eq!(options.len(), 1);
}

#[test]
fn vintage_turns_monster_card_rewards_into_relics() {
    let run = run_with(&[Modifier::Vintage]);
    let mut state = run.state().clone();
    let mut rewards = vec![Reward::Gold(5)];
    let mut pending = Vec::new();
    modify_rewards_late(
        &mut state,
        Some(RoomType::Monster),
        &mut rewards,
        &mut pending,
    );
    assert!(rewards.iter().all(|r| !matches!(r, Reward::Card(_))));
    let mut elite = vec![Reward::Gold(5)];
    modify_rewards_late(&mut state, Some(RoomType::Elite), &mut elite, &mut pending);
    assert_eq!(elite, [Reward::Gold(5)]);
}

#[test]
fn night_terrors_heal_to_full_and_cost_max_hp() {
    let mut run = run_with(&[Modifier::NightTerrors]);
    let s = run.loadout_state().expect("state");
    let max = s.max_hp;
    assert_eq!(
        modify_rest_site_heal_amount(s, Decimal::from(1)),
        Decimal::from(max)
    );
    after_rest_site_heal(s, false);
    assert_eq!(s.max_hp, max - 5);
    after_rest_site_heal(s, true);
    assert_eq!(s.max_hp, max - 5);
    s.max_hp = 3;
    after_rest_site_heal(s, false);
    assert_eq!(s.max_hp, 1);
}

#[test]
fn murderous_gives_everyone_three_strength() {
    let flags = CombatModifiers::of(&[Modifier::Murderous]);
    assert!(flags.murderous && !flags.terminal);
    let mut run = run_with(&[Modifier::Murderous]);
    let combat = fight(&mut run);
    assert!(
        combat
            .state()
            .enemies
            .iter()
            .all(|&e| { combat.state().creature(e).power_amount(PowerKind::Strength) == 3 })
    );
    let player = combat.state().player.creature;
    assert_eq!(
        combat
            .state()
            .creature(player)
            .power_amount(PowerKind::Strength),
        3
    );
}

#[test]
fn terminal_gives_plating_and_costs_a_max_hp() {
    let mut run = run_with(&[Modifier::Terminal]);
    let fresh = run_with(&[]).state().max_hp;
    assert_eq!(run.state().max_hp, fresh - 1);
    let combat = fight(&mut run);
    let player = combat.state().player.creature;
    assert_eq!(
        combat
            .state()
            .creature(player)
            .power_amount(PowerKind::Plating),
        5
    );
}

#[test]
fn deadly_events_raise_the_elite_odds() {
    let plain = run_with(&[]);
    let deadly = run_with(&[Modifier::DeadlyEvents]);
    assert_ne!(plain.state().unknown_odds, deadly.state().unknown_odds);
    let in_bag = |bag: &crate::run::relic_bag::RelicGrabBag| {
        bag.deques
            .iter()
            .any(|(_, ids)| ids.contains(&"JUZU_BRACELET"))
    };
    assert!(!in_bag(&deadly.state().player_relic_bag) && !in_bag(&deadly.state().shared_relic_bag));
}

#[test]
fn cursed_run_adds_a_curse_each_act() {
    let run = run_with(&[Modifier::CursedRun]);
    let curses = run
        .state()
        .deck
        .iter()
        .filter(|c| {
            CardKind::ALL
                .iter()
                .any(|k| k.entry() == c.id && k.card_type() == CardType::Curse)
        })
        .count();
    assert_eq!(curses, 1);
}

#[test]
fn hoarder_triples_each_added_card() {
    let mut run = run_with(&[Modifier::Hoarder]);
    let s = run.loadout_state().expect("state");
    let before = s.deck.len();
    s.add_card(RunCard::new("BIG_BANG"));
    assert_eq!(s.deck.len(), before + 3);
    assert!(!allows_merchant_card_removal(s));
}

#[test]
fn all_star_adds_five_colorless_cards() {
    let mut run = run_with(&[Modifier::AllStar]);
    take_neow(&mut run);
    assert_eq!(run.state().deck.len(), starter_len() + 5);
}

#[test]
fn specialized_adds_one_card_five_times() {
    let mut run = run_with(&[Modifier::Specialized]);
    take_neow(&mut run);
    let deck = &run.state().deck;
    assert_eq!(deck.len(), starter_len() + 5);
    let extra = &deck[starter_len()..];
    assert!(extra.iter().all(|c| c.id == extra[0].id));
}

#[test]
fn insanity_replaces_the_deck_with_thirty_cards() {
    let mut run = run_with(&[Modifier::Insanity]);
    assert!(run.state().deck.is_empty() || run.state().deck.len() < starter_len());
    take_neow(&mut run);
    assert_eq!(run.state().deck.len(), 30);
}

#[test]
fn draft_offers_ten_unskippable_card_rewards() {
    let mut run = run_with(&[Modifier::Draft]);
    take_neow(&mut run);
    for i in 0..10 {
        assert!(
            matches!(&run.phase, Phase::Rewards(r) if r.len() == 1),
            "reward {i}"
        );
        let actions = run.legal_actions();
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, RunAction::SkipReward(_) | RunAction::Proceed))
        );
        run.apply(RunAction::PickCard { reward: 0, card: 0 })
            .expect("pick");
    }
    assert_eq!(run.state().deck.len(), 10);
    assert!(!matches!(run.phase, Phase::Rewards(_)));
}

#[test]
fn sealed_deck_keeps_ten_of_thirty() {
    let mut run = run_with(&[Modifier::SealedDeck]);
    take_neow(&mut run);
    match &run.phase {
        Phase::CardSelect(select) => {
            assert_eq!(select.offered.len(), 30);
            assert_eq!(select.count, 10);
        }
        _ => panic!("a card selection"),
    }
    for i in 0..10 {
        run.apply(RunAction::SelectCard(i)).expect("select");
    }
    // The picks are final at the confirm button (`RequireManualConfirmation`).
    assert_eq!(run.state().deck.len(), 0);
    run.apply(RunAction::ConfirmSelect).expect("confirm");
    assert_eq!(run.state().deck.len(), 10);
}

#[test]
fn a_sealed_pick_can_be_taken_back_until_confirmed() {
    let mut run = run_with(&[Modifier::SealedDeck]);
    take_neow(&mut run);
    let Phase::CardSelect(select) = &run.phase else {
        panic!("a card selection");
    };
    let offered = select.offered.clone();
    for i in 0..10 {
        run.apply(RunAction::SelectCard(i)).expect("select");
    }
    // Full: no more picks and no confirm until it has all ten, and a pick can be taken back.
    assert_eq!(
        run.apply(RunAction::SelectCard(10)),
        Err(RunError::IllegalAction)
    );
    run.apply(RunAction::DeselectCard(3)).expect("deselect");
    assert_eq!(
        run.apply(RunAction::ConfirmSelect),
        Err(RunError::IllegalAction)
    );
    assert_eq!(
        run.apply(RunAction::DeselectCard(3)),
        Err(RunError::IllegalAction)
    );
    run.apply(RunAction::SelectCard(10)).expect("another");
    run.apply(RunAction::ConfirmSelect).expect("confirm");
    let mut deck = run.state().deck.clone();
    let mut want: Vec<RunCard> = (0..10)
        .filter(|&i| i != 3)
        .chain([10])
        .map(|i| offered[i])
        .collect();
    deck.sort_by_key(|c| c.id);
    want.sort_by_key(|c| c.id);
    assert_eq!(deck, want);
}

#[test]
fn big_game_hunter_maps_have_more_elites() {
    let count = |run: &Run| {
        run.state()
            .map
            .points()
            .iter()
            .filter(|p| p.kind == crate::run::MapPointType::Elite)
            .count()
    };
    let plain = count(&run_with(&[]));
    let hunter = count(&run_with(&[Modifier::BigGameHunter]));
    assert!(hunter > plain, "{hunter} elites against {plain}");
}

#[test]
fn character_cards_widen_the_card_reward_pool() {
    let run = run_with(&[Modifier::CharacterCards(Character::Ironclad)]);
    assert_eq!(
        merchant_extra_character(run.state()),
        Some(Character::Ironclad)
    );
    let options = modify_card_reward_creation_options(
        run.state(),
        CardCreation::for_room(crate::run::odds::CardRarityOddsType::RegularEncounter),
    );
    assert_eq!(
        options.pool,
        CardPool::CharacterAnd(ExtraPools::of(Character::Ironclad))
    );
}

#[test]
fn several_character_cards_union_their_pools_in_order() {
    let run = run_with(&[
        Modifier::CharacterCards(Character::Ironclad),
        Modifier::CharacterCards(Character::Silent),
        Modifier::CharacterCards(Character::Ironclad),
    ]);
    let options = modify_card_reward_creation_options(
        run.state(),
        CardCreation::for_room(crate::run::odds::CardRarityOddsType::RegularEncounter),
    );
    let wanted = ExtraPools::of(Character::Ironclad).with(Character::Silent);
    assert_eq!(options.pool, CardPool::CharacterAnd(wanted));
    assert_eq!(
        wanted.iter().collect::<Vec<_>>(),
        [Character::Ironclad, Character::Silent]
    );
}

fn fight(run: &mut Run) -> &Combat {
    if let Phase::Ancient(_) = run.phase {
        run.apply(RunAction::AncientOption(0)).ok();
    }
    let node = run
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, RunAction::ChooseNode(_)))
        .expect("a node");
    run.apply(node).expect("travel");
    run.combat().expect("a fight")
}

#[test]
fn pick_two_good_one_bad_respects_exclusions() {
    for seed in 0..50 {
        let mut rng = Rng::new(seed);
        let picked = Modifier::pick_2_good_1_bad(&mut rng, &[]);
        assert_eq!(picked.len(), 3);
        assert!(picked[0].is_good() && picked[1].is_good() && !picked[2].is_good());
        assert!(!picked[0].excludes(picked[1]) && !picked[1].excludes(picked[0]));
        let _ = CardKind::ALL.len();
    }
}

#[test]
fn the_daily_picks_hero_ascension_seed_and_modifiers_from_the_date() {
    let a = Modifier::daily_run("04_11_2026");
    assert_eq!(a, Modifier::daily_run("04_11_2026"));
    assert_eq!(a.seed, "04_11_2026_1P");
    assert!(a.ascension <= 10);
    assert_eq!(a.modifiers.len(), 3);
    assert!(a.modifiers[0].is_good() && a.modifiers[1].is_good() && !a.modifiers[2].is_good());
    // No CharacterCards of the rolled hero.
    assert!(
        a.modifiers
            .iter()
            .all(|m| m.character() != Some(a.character))
    );
    let days: std::collections::HashSet<_> = (1..=28)
        .map(|d| Modifier::daily_run(&format!("{d:02}_11_2026")))
        .map(|r| (r.character, r.ascension))
        .collect();
    assert!(days.len() > 5, "the date moves the hero and ascension");
}

#[test]
fn big_game_hunter_regenerates_a_spoils_map_from_its_own_stream() {
    let run = run_with(&[]);
    let spoils = ActMap::generate_spoils(&run.state().rng, 1, 0).expect("spoils map");
    let elites = |m: &ActMap| {
        m.points()
            .iter()
            .filter(|p| p.kind == crate::run::MapPointType::Elite)
            .count()
    };
    let hunted =
        ActMap::generate_big_game_hunter(&run.state().rng, 1, 0, &spoils).expect("hunter map");
    assert!(hunted.is_spoils());
    // `Math.Round(n * 2.5f)` rounds halves to even.
    let halves = elites(&spoils) * 5;
    let wanted = halves / 2 + usize::from(halves % 2 == 1 && (halves / 2) % 2 == 1);
    // The hourglass has fewer points, and pruning drops some after the elites are placed.
    assert!((elites(&spoils) + 1..=wanted).contains(&elites(&hunted)));
    assert!(elites(&hunted) > elites(&spoils));
    assert_ne!(hunted, spoils);
}

#[test]
fn the_sealed_grid_is_sorted_by_rarity_then_title() {
    let card = |id| RunCard::new(id);
    let cards = [
        card("STRIKE_NECROBINDER"),
        card("DEFEND_NECROBINDER"),
        card("BLIGHT_STRIKE"),
    ];
    let title = |c: RunCard| {
        match c.id {
            "STRIKE_NECROBINDER" => "Strike",
            "DEFEND_NECROBINDER" => "Defend",
            _ => "Blight Strike",
        }
        .to_owned()
    };
    let order = sealed_deck::display_order(&cards, title);
    let ids: Vec<_> = order.iter().map(|&i| cards[i].id).collect();
    // Basics first (Defend before Strike), then the common.
    assert_eq!(
        ids,
        ["DEFEND_NECROBINDER", "STRIKE_NECROBINDER", "BLIGHT_STRIKE"]
    );
}

#[test]
fn hoarder_does_not_copy_a_transformed_card() {
    // `Transform` reports the card's old pile as the Deck, and `Hoarder` copies only a card that
    // came from no pile.
    let mut run = run_with(&[Modifier::Hoarder]);
    let s = run.loadout_state().expect("state");
    let before = s.deck.len();
    s.transform_to(0, RunCard::new("BIG_BANG"));
    assert_eq!(s.deck.len(), before);
    s.transform_many(&[(0, RunCard::new("BIG_BANG")), (1, RunCard::new("BIG_BANG"))]);
    assert_eq!(s.deck.len(), before);
}
