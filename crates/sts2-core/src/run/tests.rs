//! Run init and room flow against the referee's `run_act1` example (seed `T6S0`).

use super::*;
use crate::content::cards::CardType::{Attack, Power, Skill};
use crate::content::enchantments::EnchantmentKind;
use crate::run::odds::CardRarity;

fn t6s0() -> Run {
    Run::new(
        Character::Necrobinder,
        "T6S0",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("T6S0 starts")
}

#[test]
fn init_matches_the_referee() {
    let run = t6s0();
    let s = run.state();
    assert_eq!(s.rng[RunRngType::UpFront].counter(), 417);
    let act = s.act();
    let normals: Vec<&str> = act.normal_encounters.iter().map(|e| e.id).collect();
    assert_eq!(
        normals[..4],
        [
            "SHRINKER_BEETLE_WEAK",
            "FUZZY_WURM_CRAWLER_WEAK",
            "NIBBITS_WEAK",
            "VINE_SHAMBLER_NORMAL"
        ]
    );
    assert_eq!(
        act.events[..3],
        ["BRAIN_LEECH", "WAR_HISTORIAN_REPY", "JUNGLE_MAZE_ADVENTURE"]
    );
    assert_eq!(act.elite_encounters[0].id, "PHROG_PARASITE_ELITE");
    assert_eq!(act.boss.id, "VANTOM_BOSS");
    assert_eq!(act.ancient, "NEOW");
    assert_eq!(act.events_visited, 1);
    assert_eq!(s.total_floor(), 1);
    assert_eq!(
        run.ancient_relics(),
        ["PHIAL_HOLSTER", "BOOMING_CONCH", "NEOWS_BONES"]
    );
}

#[test]
fn kaleidoscope_offers_two_card_rewards_from_other_pools() {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S15",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("RL2S15 starts");
    assert_eq!(
        run.ancient_relics(),
        ["NEW_LEAF", "KALEIDOSCOPE", "LARGE_CAPSULE"]
    );
    run.apply(RunAction::AncientOption(1))
        .expect("Kaleidoscope is taken");
    let Phase::Rewards(rewards) = &run.phase else {
        panic!("Kaleidoscope offers a rewards screen");
    };
    assert_eq!(rewards.len(), 2);
    for reward in rewards {
        let Reward::Card(cards) = reward else {
            panic!("a card reward");
        };
        // One card from each of 3 different pools other than the Necrobinder's.
        let owners: Vec<_> = cards
            .cards
            .iter()
            .map(|c| Character::of_card(c.id).expect("a character card"))
            .collect();
        assert_eq!(owners.len(), 3);
        assert!(!owners.contains(&Character::Necrobinder));
        assert!(
            owners
                .iter()
                .enumerate()
                .all(|(i, o)| !owners[..i].contains(o))
        );
    }
}

/// Seed `RL2S4` as the referee plays it: Golden Pearl, then the row-1 fight at `(1, 1)`.
#[test]
fn neow_then_first_fight_matches_the_referee() {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S4",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("RL2S4 starts");
    assert_eq!(
        run.ancient_relics(),
        ["NEOWS_TALISMAN", "GOLDEN_PEARL", "DOWSING_ROD"]
    );
    run.apply(RunAction::AncientOption(1))
        .expect("Golden Pearl is ported");
    assert_eq!(run.state().gold, 99 + 150);
    let relics: Vec<&str> = run.state().relics.iter().map(|r| r.id()).collect();
    assert_eq!(relics, ["BOUND_PHYLACTERY", "GOLDEN_PEARL"]);
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));

    run.apply(RunAction::ChooseNode(MapCoord { col: 1, row: 1 }))
        .expect("the first fight is ported");
    let s = run.state();
    assert_eq!((s.total_floor(), s.act_floor), (2, 2));
    assert_eq!(s.act().normal_encounters_visited, 1);
    let combat = run.combat().expect("in combat");
    let cs = combat.state();
    assert_eq!(cs.map_coord, Some((1, 1)));
    assert_eq!(cs.rng[RunRngType::Shuffle].counter(), 9);
    assert_eq!(cs.rng[RunRngType::Niche].counter(), 1);
}

#[test]
fn wrong_phase_actions_are_rejected() {
    let mut run = t6s0();
    assert_eq!(
        run.apply(RunAction::ClaimReward(0)),
        Err(RunError::IllegalAction)
    );
    assert_eq!(run.apply(RunAction::Proceed), Err(RunError::IllegalAction));
}

/// `T6S0` as in `referee/examples/run_act1.trace.jsonl`: Phial Holster's potions, the first
/// fight's rewards (gold 10, Heart of Iron, pity -0.02, 13 `rewards` draws), and the `?` rolls.
#[test]
fn phial_holster_and_first_rewards_match_the_referee() {
    let mut run = t6s0();
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    let s = run.state();
    assert_eq!(
        s.potions,
        [Some("BOTTLED_POTENTIAL"), Some("WEAK_POTION"), None, None]
    );
    assert_eq!(s.rng[RunRngType::CombatPotionGeneration].counter(), 4);

    // The rewards of a won monster fight depend only on the player's streams and odds.
    let mut state = run.state().clone();
    let rewards = rewards::generate_combat_rewards(
        &mut state,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    assert_eq!(rewards[0], Reward::Gold(10));
    assert_eq!(rewards[1], Reward::Potion("HEART_OF_IRON"));
    let Reward::Card(cards) = &rewards[2] else {
        panic!("a card reward")
    };
    assert_eq!(cards.cards[0].id, "DEFY");
    assert_eq!(state.player_rng[PlayerRngType::Rewards].counter(), 13);
    assert_eq!(
        state.card_rarity_odds.offset.to_bits(),
        (-0.02_f32).to_bits()
    );
}

/// `RewardsSet.cs:225-228`: a monster room's gold scales by `GoldProportion` and is skipped at 0
/// (with no `rewards` draw); an elite ignores it.
#[test]
fn escaped_monsters_scale_the_gold_reward() {
    use odds::GoldProportion;
    let run = t6s0();
    let draws = |s: &RunState| s.player_rng[PlayerRngType::Rewards].counter();
    let gold = |rewards: &[Reward]| {
        rewards.iter().find_map(|r| match r {
            Reward::Gold(n) => Some(*n),
            _ => None,
        })
    };
    let mut full = run.state().clone();
    let all = rewards::generate_combat_rewards(
        &mut full,
        RoomType::Monster,
        GoldProportion::ONE,
        Vec::new(),
    );
    let mut half = run.state().clone();
    let some = rewards::generate_combat_rewards(
        &mut half,
        RoomType::Monster,
        GoldProportion::HALF,
        Vec::new(),
    );
    // `NextInt(5, 11)` instead of `NextInt(10, 21)`: same draw count.
    assert!(gold(&some).is_some_and(|g| (5..=10).contains(&g)));
    assert!(gold(&all).is_some_and(|g| (10..=20).contains(&g)));
    assert_eq!(draws(&half), draws(&full));
    let mut none = run.state().clone();
    let rewards = rewards::generate_combat_rewards(
        &mut none,
        RoomType::Monster,
        GoldProportion::ZERO,
        Vec::new(),
    );
    assert_eq!(gold(&rewards), None);
    assert_eq!(draws(&none) + 1, draws(&full));
    let mut elite = run.state().clone();
    let rewards = rewards::generate_combat_rewards(
        &mut elite,
        RoomType::Elite,
        GoldProportion::ZERO,
        Vec::new(),
    );
    assert!(gold(&rewards).is_some_and(|g| (35..=45).contains(&g)));
}

/// `T6S0` as in the example trace, steps 30-32: after the first fight's rewards, the first `?`
/// rolls an event and the second a treasure room with `MOLTEN_EGG` and 42 gold.
#[test]
fn unknown_rolls_and_treasure_match_the_referee() {
    let mut run = t6s0();
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    let mut s = run.state().clone();
    rewards::generate_combat_rewards(
        &mut s,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    let unknown = &mut s.rng[RunRngType::UnknownMapPoint];
    assert_eq!(
        s.unknown_odds
            .roll(&UnknownMapPointOdds::room_types(), unknown),
        RoomType::Event
    );
    assert_eq!(
        s.unknown_odds
            .roll(&UnknownMapPointOdds::room_types(), unknown),
        RoomType::Treasure
    );
    let shop = s.unknown_odds.get(RoomType::Shop).expect("shop odds");
    assert_eq!(shop.to_string(), "0.089999996");
    assert_eq!(rewards::roll_treasure_relic(&mut s), "MOLTEN_EGG");
    assert_eq!(
        s.player_rng[PlayerRngType::Rewards].next_int_range(42, 53),
        42
    );
    assert_eq!(s.rng[RunRngType::TreasureRoomRelics].counter(), 1);
}

/// A rest-site heal truncates `MaxHp × 0.3` and Stone Humidifier then adds 5 max HP.
#[test]
fn rest_heal_truncates() {
    let mut run = t6s0();
    run.state.hp = 40;
    let _ = run.state.rest_site_heal(false);
    assert_eq!(run.state.hp, 59);
    run.state
        .relics
        .push(RunRelic::new(RelicKind::StoneHumidifier));
    run.state.hp = 10;
    let _ = run.state.rest_site_heal(false);
    assert_eq!((run.state.hp, run.state.max_hp), (34, 71));
}

/// `T6S0` after Neow (Phial Holster) with `kinds` owned.
fn with_relics(kinds: &[RelicKind]) -> Run {
    let mut run = t6s0();
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    run.state
        .relics
        .extend(kinds.iter().map(|&k| RunRelic::new(k)));
    run
}

#[test]
fn prayer_wheel_adds_a_monster_card_reward_populated_after_the_room_rewards() {
    let run = with_relics(&[]);
    let mut plain = run.state().clone();
    let base = rewards::generate_combat_rewards(
        &mut plain,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    let run = with_relics(&[RelicKind::PrayerWheel]);
    let mut s = run.state().clone();
    let rewards = rewards::generate_combat_rewards(
        &mut s,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    // The room's rewards draw first and are unchanged; the extra card reward comes last.
    assert_eq!(rewards[..3], base[..]);
    let Reward::Card(extra) = &rewards[3] else {
        panic!("a second card reward");
    };
    assert_eq!(extra.cards.len(), 3);
    // Three more rarity, pick and upgrade draws.
    assert_eq!(
        s.player_rng[PlayerRngType::Rewards].counter(),
        plain.player_rng[PlayerRngType::Rewards].counter() + 9
    );
    // Not in elite rooms.
    let mut elite = with_relics(&[RelicKind::PrayerWheel]).state().clone();
    let rewards = rewards::generate_combat_rewards(
        &mut elite,
        RoomType::Elite,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    assert_eq!(
        rewards
            .iter()
            .filter(|r| matches!(r, Reward::Card(_)))
            .count(),
        1
    );
}

#[test]
fn amethyst_aubergine_and_white_star_add_rewards() {
    let mut s = with_relics(&[RelicKind::AmethystAubergine, RelicKind::WhiteStar])
        .state()
        .clone();
    let rewards = rewards::generate_combat_rewards(
        &mut s,
        RoomType::Elite,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    // Sorted: gold, gold (15), [potion], relic, cards, cards (boss odds: all rare).
    assert_eq!(rewards[1], Reward::Gold(15));
    let Some(Reward::Card(boss)) = rewards.last() else {
        panic!("White Star's card reward");
    };
    assert!(
        boss.cards
            .iter()
            .all(|c| c.info().is_some_and(|i| i.rarity == CardRarity::Rare))
    );
}

#[test]
fn gold_hooks_bowler_hat_then_dragon_fruit() {
    let mut run = with_relics(&[RelicKind::BowlerHat, RelicKind::DragonFruit]);
    let s = &mut run.state;
    let (gold, max_hp) = (s.gold, s.max_hp);
    s.gain_gold(Decimal::from(15));
    // 15 × 1.25 = 18.75, truncated; then 1 max HP.
    assert_eq!((s.gold, s.max_hp), (gold + 18, max_hp + 1));
    // No gain, no Dragon Fruit.
    s.gain_gold(Decimal::ZERO);
    assert_eq!(s.max_hp, max_hp + 1);
}

#[test]
fn juzu_bracelet_keeps_monsters_out_of_unknown_rooms() {
    let s = with_relics(&[RelicKind::JuzuBracelet]).state().clone();
    let mut types = UnknownMapPointOdds::room_types();
    hooks::modify_unknown_map_point_room_types(&s, &mut types);
    assert!(!types.contains(&RoomType::Monster));
    // A roll that would have been a monster is an event, and the monster odds do not grow.
    let mut odds = UnknownMapPointOdds::default();
    let mut rng = crate::rng::Rng::new(1);
    for _ in 0..50 {
        assert_ne!(odds.roll(&types, &mut rng), RoomType::Monster);
    }
    assert_eq!(odds.get(RoomType::Monster), Some(0.1));
}

#[test]
fn rest_site_relics_heal_more() {
    let mut run = with_relics(&[RelicKind::RegalPillow]);
    run.state.hp = 10;
    let _ = run.state.rest_site_heal(false);
    // `66 × 0.3m + 15` = 34.8.
    assert_eq!(run.state.hp, 10 + 34);
    let mut s = with_relics(&[RelicKind::EternalFeather]).state().clone();
    s.hp = 10;
    hooks::after_room_entered(&mut s, RoomType::RestSite);
    // 10 cards: 2 × 3.
    assert_eq!(s.hp, 16);
    hooks::after_room_entered(&mut s, RoomType::Treasure);
    assert_eq!(s.hp, 16);
}

#[test]
fn obtaining_through_a_reward_runs_after_obtained_hooks() {
    let mut run = with_relics(&[RelicKind::DragonFruit]);
    let max_hp = run.state.max_hp;
    run.phase = Phase::Rewards(vec![Reward::Relic("OLD_COIN")]);
    run.apply(RunAction::ClaimReward(0))
        .expect("Old Coin is ported");
    // Old Coin's gold goes through `GainGold`, which feeds Dragon Fruit.
    assert_eq!(run.state.max_hp, max_hp + 1);
    assert_eq!(
        run.state.relics.last().map(|r| r.kind),
        Some(RelicKind::OldCoin)
    );
}

#[test]
fn relic_remove_and_replace() {
    let mut run = with_relics(&[RelicKind::Anchor, RelicKind::Strawberry]);
    let max_hp = run.state.max_hp;
    let mut ctx = RunCtx::new(&mut run);
    // Replace Anchor (index 2) with a Mango: it takes Anchor's place and its `AfterObtained`
    // runs.
    ctx.replace_relic(2, "MANGO").expect("Mango is ported");
    ctx.remove_relic(1);
    let kinds: Vec<RelicKind> = run.state.relics.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [
            RelicKind::BoundPhylactery,
            RelicKind::Mango,
            RelicKind::Strawberry
        ]
    );
    assert_eq!(run.state.max_hp, max_hp + 14);
}

#[test]
fn deck_commands() {
    let mut s = t6s0().state().clone();
    // Transform the first Strike: a Common, Uncommon or Rare Necrobinder card joins the end.
    let mut rng = crate::rng::Rng::new(7);
    s.transform_to_random(0, &mut rng).expect("a pool card");
    assert_eq!(s.deck.len(), 10);
    let new = s.deck[9];
    let info = new.info().expect("a Necrobinder pool card");
    assert!(matches!(
        info.rarity,
        CardRarity::Common | CardRarity::Uncommon | CardRarity::Rare
    ));
    assert_eq!(rng.counter(), 1);
    // A curse's pool is not modeled.
    s.add_card(RunCard::new("POOR_SLEEP"));
    assert_eq!(
        s.transform_to_random(10, &mut rng),
        Err(RunError::UnsupportedCard("POOR_SLEEP"))
    );
    // Removal lists curses first; transformation skips nothing here.
    assert_eq!(s.removable_cards()[0], 10);
    assert_eq!(s.transformable_cards().len(), 11);
    // Upgrade, downgrade.
    s.deck[0].upgrade();
    s.downgrade(0);
    assert_eq!(s.deck[0].upgrade_level, 0);
    // An unported card (a made-up id, so porting never breaks this) makes the deck unenchantable.
    s.add_card(RunCard::new("TEST_UNPORTED_CARD"));
    assert!(s.enchantable_cards(EnchantmentKind::Vigorous).is_err());
    // Drop the curse and the (maybe unported) replacement.
    s.deck.truncate(9);
    let strikes = s
        .enchantable_cards(EnchantmentKind::Vigorous)
        .expect("ported deck");
    assert!(strikes.contains(&0));
    s.enchant(0, EnchantmentKind::Vigorous, 3).expect("ported");
    assert_eq!(s.deck[0].enchantment.map(|e| e.amount), Some(3));
    let left = s
        .enchantable_cards(EnchantmentKind::Vigorous)
        .expect("ported deck");
    assert!(!left.contains(&0));
    // The enchantment goes into combat with the card.
    assert_eq!(
        s.deck[0].to_combat().and_then(|c| c.enchantment),
        s.deck[0].enchantment
    );
}

#[test]
fn unported_potions_keep_their_combat_slots() {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S4",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("RL2S4 starts");
    run.apply(RunAction::AncientOption(1))
        .expect("Golden Pearl is ported");
    run.state.potions = vec![Some("DEPRECATED_POTION"), None, Some("FIRE_POTION")];
    run.apply(RunAction::ChooseNode(MapCoord { col: 1, row: 1 }))
        .expect("the first fight is ported");
    let player = &run.combat().expect("in combat").state().player;
    assert_eq!(
        player.unported_potions,
        [Some("DEPRECATED_POTION"), None, None]
    );
    assert_eq!(
        player.potions[2].map(|p| p.kind),
        Some(crate::content::potions::PotionKind::FirePotion)
    );
    assert_eq!(player.first_empty_potion_slot(), Some(1));
    assert_eq!(
        run.potions(),
        [Some("DEPRECATED_POTION"), None, Some("FIRE_POTION")]
    );
}

#[test]
fn fruit_juice_outside_combat() {
    let mut run = with_relics(&[]);
    run.state.potions = vec![None, Some("FRUIT_JUICE"), Some("WEAK_POTION")];
    run.forget_journal();
    let max_hp = run.state.max_hp;
    let actions = run.legal_actions();
    assert!(actions.contains(&RunAction::UsePotion(1)));
    // Weak Potion is `CombatOnly` (and unported).
    assert!(!actions.contains(&RunAction::UsePotion(2)));
    run.apply(RunAction::UsePotion(1)).expect("Fruit Juice");
    assert_eq!(run.state.max_hp, max_hp + 5);
    assert_eq!(run.state.potions, [None, None, Some("WEAK_POTION")]);
    assert_eq!(
        run.apply(RunAction::UsePotion(1)),
        Err(RunError::IllegalAction)
    );
}

/// A rejected action that failed after changing the run (a fight cannot start with an unported
/// deck card, found once the room is entered) leaves the run exactly as before, through the
/// journal's checkpoint and the replay of the actions since (`rollback`); the run goes on.
#[test]
fn a_rejected_action_is_rolled_back_by_replay() {
    let mut run = with_relics(&[]);
    run.state.potions = vec![Some("FRUIT_JUICE"), Some("FRUIT_JUICE"), None];
    run.state.deck.push(RunCard::new("NOT_A_CARD"));
    run.forget_journal();
    run.apply(RunAction::UsePotion(0)).expect("Fruit Juice");
    run.apply(RunAction::DiscardPotion(1))
        .expect("a potion to discard");
    assert_eq!(run.journal_len(), 2);
    let before = run.clone();
    let node = run.next_nodes()[0];
    let mut in_place = run.clone();
    assert_eq!(
        in_place.apply_out_of_combat(RunAction::ChooseNode(node)),
        Err(RunError::UnsupportedCard("NOT_A_CARD"))
    );
    assert_ne!(in_place, before, "the failure comes after a change");
    assert_eq!(
        run.apply(RunAction::ChooseNode(node)),
        Err(RunError::UnsupportedCard("NOT_A_CARD"))
    );
    assert_eq!(run, before);
    assert_eq!(run.journal_len(), 2);
    run.state.deck.pop();
    run.forget_journal();
    run.apply(RunAction::ChooseNode(node)).expect("a fight");
    assert!(run.combat().is_some());
}

#[test]
fn potions_can_be_discarded_on_the_map_and_in_combat() {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S4",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("RL2S4 starts");
    run.apply(RunAction::AncientOption(1))
        .expect("Golden Pearl is ported");
    run.state.potions = vec![Some("FRUIT_JUICE"), None, Some("FIRE_POTION")];
    assert_eq!(
        run.discard_actions(),
        [RunAction::DiscardPotion(0), RunAction::DiscardPotion(2)]
    );
    assert!(!run.legal_actions().contains(&RunAction::DiscardPotion(0)));
    assert_eq!(
        run.apply(RunAction::DiscardPotion(1)),
        Err(RunError::IllegalAction)
    );
    run.apply(RunAction::DiscardPotion(0)).expect("discard");
    assert_eq!(run.state.potions, [None, None, Some("FIRE_POTION")]);

    run.apply(RunAction::ChooseNode(MapCoord { col: 1, row: 1 }))
        .expect("the first fight is ported");
    assert_eq!(run.discard_actions(), [RunAction::DiscardPotion(2)]);
    run.apply(RunAction::DiscardPotion(2)).expect("discard");
    assert_eq!(run.potions(), [None, None, None]);
    assert_eq!(run.discard_actions(), []);
    assert_eq!(
        run.apply(RunAction::DiscardPotion(2)),
        Err(RunError::IllegalAction)
    );
}

#[test]
fn ascenders_bane_joins_the_starting_deck_from_ascension_5() {
    assert_eq!(starting_deck(Character::Necrobinder, 4).len(), 10);
    let deck = starting_deck(Character::Necrobinder, 5);
    assert_eq!(deck.last().map(|c| c.id), Some("ASCENDERS_BANE"));
    assert!(!deck[10].is_removable());
}

/// `T6S0` after Neow in a shop generated on the spot, with `kinds` owned and `gold`.
fn in_shop(kinds: &[RelicKind], gold: i32) -> Run {
    let mut run = with_relics(kinds).in_test_mode();
    run.state.gold = gold;
    run.state.room = Room::Shop;
    let shop = Shop::generate(&mut run.state);
    run.phase = Phase::Shop(Box::new(shop));
    run
}

fn shop_of(run: &Run) -> &Shop {
    match run.pending_decision() {
        RunDecision::Shop(shop) => shop,
        d => panic!("not in a shop: {d:?}"),
    }
}

#[test]
fn shop_inventory_draws_and_prices() {
    let run = with_relics(&[]);
    let mut game = run.state().clone();
    let mut test = run.state().clone();
    test.test_mode = true;
    let rewards_before = game.player_rng[PlayerRngType::Rewards].counter();
    let shop = Shop::generate(&mut game);
    let test_shop = Shop::generate(&mut test);
    // 5 character cards (rarity, upgrade) + 2 colorless (upgrade) + 2 relic rarities.
    assert_eq!(
        game.player_rng[PlayerRngType::Rewards].counter() - rewards_before,
        14
    );
    // Sale index, 7 × (pick, price), one more price for the sale, 3 relic prices, 3 potions
    // (rarity, pick) and, outside test mode, 3 potion prices.
    assert_eq!(
        game.player_rng[PlayerRngType::Shops].counter(),
        1 + 14 + 1 + 3 + 6 + 3
    );
    assert_eq!(
        test.player_rng[PlayerRngType::Shops].counter(),
        1 + 14 + 1 + 3 + 6
    );
    assert_eq!(game.card_rarity_odds, run.state().card_rarity_odds);
    assert_eq!(shop.cards, test_shop.cards);
    assert_eq!(shop.relics, test_shop.relics);
    let types: Vec<_> = shop.cards[..5]
        .iter()
        .map(|c| c.card.and_then(RunCard::card_type))
        .collect();
    assert_eq!(
        types,
        [Attack, Attack, Skill, Skill, Power].map(Some).to_vec()
    );
    assert_eq!(shop.cards.iter().filter(|c| c.on_sale).count(), 1);
    for p in &test_shop.potions {
        assert!([50, 75, 100].contains(&p.base_cost));
    }
    assert_eq!(shop.removal_base_cost, 75);
    for r in &shop.relics {
        let id = r.id.expect("stocked");
        assert!(
            !game
                .player_relic_bag
                .deques
                .iter()
                .any(|(_, l)| l.contains(&id))
        );
        assert!(
            !game
                .shared_relic_bag
                .deques
                .iter()
                .any(|(_, l)| l.contains(&id))
        );
    }
}

#[test]
fn merchant_discounts_truncate() {
    let run = with_relics(&[RelicKind::MembershipCard]);
    assert_eq!(shop::price(run.state(), 75), 37);
    let run = with_relics(&[RelicKind::TheCourier]);
    assert_eq!(shop::price(run.state(), 77), 61);
    let run = with_relics(&[RelicKind::MembershipCard, RelicKind::TheCourier]);
    assert_eq!(shop::price(run.state(), 75), 30);
}

#[test]
fn buying_and_removing_in_a_shop() {
    let mut run = in_shop(&[], 1000);
    let deck = run.state().deck.len();
    let card = shop_of(&run).cards[1].clone();
    run.apply(RunAction::BuyCard(1)).expect("affordable");
    assert_eq!(run.state().deck.len(), deck + 1);
    assert_eq!(run.state().gold, 1000 - card.base_cost);
    assert_eq!(shop_of(&run).cards[1].card, None);
    assert_eq!(
        run.apply(RunAction::BuyCard(1)),
        Err(RunError::IllegalAction)
    );

    let potion = shop_of(&run).potions[0].clone();
    run.apply(RunAction::BuyPotion(0)).expect("a free slot");
    assert!(run.state().potions.contains(&potion.id));

    let gold = run.state().gold;
    let removal = shop_of(&run).removal_base_cost;
    run.apply(RunAction::RemoveCard).expect("removal");
    assert!(run.legal_actions().contains(&RunAction::CancelSelect));
    run.apply(RunAction::CancelSelect).expect("cancelable");
    assert_eq!(run.state().gold, gold);
    run.apply(RunAction::RemoveCard).expect("removal");
    run.apply(RunAction::SelectCard(0)).expect("a card");
    assert_eq!(run.state().deck.len(), deck);
    assert_eq!(run.state().gold, gold - removal);
    assert_eq!(run.state().card_shop_removals_used, 1);
    assert!(shop_of(&run).removal_used);
    assert!(!run.legal_actions().contains(&RunAction::RemoveCard));
    run.apply(RunAction::Proceed).expect("leave");
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));

    // The next shop's removal costs 25 more.
    let mut s = run.state().clone();
    assert_eq!(Shop::generate(&mut s).removal_base_cost, 100);
}

#[test]
fn shop_purchases_need_gold() {
    let run = in_shop(&[], 0);
    assert_eq!(run.legal_actions(), [RunAction::Proceed]);
}

/// `MerchantPotionEntry.OnTryPurchase` fails without effect when `TryToProcure` does (a full
/// belt or a `ShouldProcurePotion` veto), so neither is offered.
#[test]
fn shop_potions_need_a_procurable_slot() {
    let buys_potion = |run: &Run| {
        run.legal_actions()
            .iter()
            .any(|a| matches!(a, RunAction::BuyPotion(_)))
    };
    assert!(buys_potion(&in_shop(&[], 1000)));
    assert!(!buys_potion(&in_shop(&[RelicKind::Sozu], 1000)));
    let mut run = in_shop(&[], 1000);
    run.state
        .potions
        .iter_mut()
        .for_each(|p| *p = Some("FIRE_POTION"));
    assert!(!buys_potion(&run));
}

#[test]
fn the_courier_restocks_bought_slots() {
    let mut run = in_shop(&[RelicKind::TheCourier], 1000);
    let card = shop_of(&run).cards[4].card;
    run.apply(RunAction::BuyCard(4)).expect("affordable");
    let restocked = &shop_of(&run).cards[4];
    assert!(restocked.card.is_some() && restocked.card != card);
    assert!(!restocked.on_sale);
    let relic = shop_of(&run).relics[0].id;
    let rewards = run.state().player_rng[PlayerRngType::Rewards].counter();
    run.apply(RunAction::BuyPotion(0)).expect("affordable");
    assert!(shop_of(&run).potions[0].id.is_some());
    assert_eq!(shop_of(&run).relics[0].id, relic);
    assert_eq!(
        run.state().player_rng[PlayerRngType::Rewards].counter(),
        rewards
    );
}

/// A shop with `TheCourier`, a full belt and `CAULDRON` in relic slot 0, bought: its five
/// potions are left on the rewards screen.
fn cauldron_bought_on_a_full_belt(test_mode: bool) -> (Run, i32) {
    let mut run = in_shop(&[RelicKind::TheCourier], 1000);
    run.state.test_mode = test_mode;
    run.state.potions.fill(Some("FLEX_POTION"));
    if let Phase::Shop(shop) = &mut run.phase {
        shop.relics[0].id = Some("CAULDRON");
    }
    let rewards = run.state().player_rng[PlayerRngType::Rewards].counter();
    run.apply(RunAction::BuyRelic(0)).expect("affordable");
    assert!(matches!(run.pending_decision(), RunDecision::Rewards(r) if r.len() == 5));
    run.apply(RunAction::Proceed).expect("leaves the rewards");
    (run, rewards)
}

#[test]
fn cauldron_left_unclaimed_completes_its_purchase() {
    // The player leaves the rewards screen with potions that do not fit, which is a skip: the
    // purchase finishes and `TheCourier` restocks the slot (`RollRarity` on `rewards`). Test
    // mode takes the same path: the referee's test selector skips what it cannot take
    // (`referee/tracer/Rewards.cs`), it no longer aborts the purchase.
    let (run, rewards) = cauldron_bought_on_a_full_belt(true);
    assert_ne!(shop_of(&run).relics[0].id, Some("CAULDRON"));
    // Test mode's five potions are fixed (no `PotionReward` rolls); only the restock draws.
    assert!(run.state().player_rng[PlayerRngType::Rewards].counter() > rewards);
    let (run, rewards) = cauldron_bought_on_a_full_belt(false);
    assert_ne!(shop_of(&run).relics[0].id, Some("CAULDRON"));
    assert!(run.state().player_rng[PlayerRngType::Rewards].counter() > rewards + 5);
}

#[test]
fn meal_ticket_heals_on_entering_a_shop() {
    let mut run = with_relics(&[RelicKind::MealTicket]);
    run.state.hp = 30;
    hooks::after_room_entered(&mut run.state, RoomType::Shop);
    assert_eq!(run.state.hp, 45);
    hooks::after_room_entered(&mut run.state, RoomType::RestSite);
    assert_eq!(run.state.hp, 45);
}

/// Leaving the Act 1 boss's rewards enters Act 2's map screen: no heal, and the only point to
/// travel to is the start (Hive's ancient).
#[test]
fn boss_rewards_enter_the_next_act() {
    let mut run = t6s0();
    run.state.room = Room::Combat {
        room_type: RoomType::Boss,
        encounter: "VANTOM_BOSS",
    };
    run.state.visited.push(run.state.map.boss().coord);
    run.state.act_floor = 17;
    run.state.unknown_odds = UnknownMapPointOdds::default();
    run.state.hp = 20;
    let floor = run.state.total_floor();
    run.phase = Phase::Rewards(Vec::new());
    run.apply(RunAction::Proceed)
        .expect("the rewards can be left");
    let start = run.state.map.start().coord;
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(smallvec::smallvec![start])
    );
    assert_eq!(run.legal_actions(), vec![RunAction::ChooseNode(start)]);
    let s = run.state();
    assert_eq!((s.act_index, s.act_floor, s.total_floor()), (1, 0, floor));
    assert_eq!(s.hp, 20);
    assert!(s.visited.is_empty() && s.coord().is_none());
    assert_eq!(s.room, Room::Map);
    assert_eq!(s.act().act.id, "HIVE");
    // Hive's map: 14 rooms, the boss on row 15.
    assert_eq!(s.map.boss().coord.row, 15);
    assert_eq!(
        s.map,
        ActMap::generate(&s.rng, 1, 0).expect("the seed's Hive map")
    );
}

/// `HeftyTablet`: the rare pick joins the deck, then `Injury`; a skip adds only `Injury`.
#[test]
fn hefty_tablet_offers_a_skippable_choice() {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S16",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("RL2S16 starts");
    assert_eq!(
        run.ancient_relics(),
        ["BOOMING_CONCH", "NEOWS_TORMENT", "HEFTY_TABLET"]
    );
    run.apply(RunAction::AncientOption(2)).expect("ported");
    let RunDecision::CardSelect(select) = run.pending_decision() else {
        panic!("a choose-a-card screen");
    };
    let offered: Vec<&str> = select.offered.iter().map(|c| c.id).collect();
    assert_eq!(offered, ["OBLIVION", "REAPER_FORM", "SOUL_STORM"]);
    let mut skipped = run.clone();
    skipped.apply(RunAction::CancelSelect).expect("skippable");
    let ids = |r: &Run| r.state().deck.iter().map(|c| c.id).collect::<Vec<_>>();
    assert_eq!(ids(&skipped)[10..], ["INJURY"]);
    assert!(matches!(skipped.pending_decision(), RunDecision::Map(_)));
    run.apply(RunAction::SelectCard(1)).expect("a pick");
    assert_eq!(ids(&run)[10..], ["REAPER_FORM", "INJURY"]);
}

/// `WingedBoots`: free travel to any point of the next row, 3 times; a child costs nothing.
#[test]
fn winged_boots_allow_free_travel_three_times() {
    let mut run = t6s0();
    run.state.relics.push(RunRelic::new(RelicKind::WingedBoots));
    run.phase = Phase::Map;
    run.state.room = Room::Map;
    run.state.visited = vec![run.state.map.start().coord];
    let times_used = |run: &Run| match run.state.relics.last().expect("the boots").data {
        RelicData::WingedBoots(d) => d.times_used,
        _ => unreachable!("WingedBoots data"),
    };
    let mut free_moves = 0;
    for row in 1..run.state.map.boss().coord.row {
        let here = *run.state.visited.last().expect("a point");
        let children = run
            .state
            .map
            .point(here)
            .expect("on the map")
            .children
            .to_vec();
        let mut points: Vec<MapCoord> = run
            .state
            .map
            .points()
            .iter()
            .filter(|p| p.coord.row == row)
            .map(|p| p.coord)
            .collect();
        points.sort();
        let mut expected = if free_moves < 3 {
            points
        } else {
            children.clone()
        };
        expected.sort();
        let next = run.next_nodes();
        assert_eq!(next[..], expected, "row {row}");
        // A point that is not a child when there is one, as `enter_node` would enter it.
        let target = next
            .iter()
            .copied()
            .find(|c| !children.contains(c))
            .unwrap_or(next[0]);
        free_moves += i32::from(!children.contains(&target));
        run.state.visited.push(target);
        hooks::after_map_room_entered(&mut run.state);
        assert_eq!(times_used(&run), free_moves);
    }
    assert_eq!(free_moves, 3);
}

/// A free travel into a fight is counted before the relics move into the combat
/// (`WingedBoots.AfterRoomEntered` is the fight room's first hook; `RL2S21` step 25).
#[test]
fn winged_boots_count_a_free_travel_into_a_fight() {
    let mut run = t6s0();
    run.state.relics.push(RunRelic::new(RelicKind::WingedBoots));
    run.phase = Phase::Map;
    run.state.room = Room::Map;
    let map = &run.state.map;
    let (from, to) = map
        .points()
        .iter()
        .filter(|p| p.coord.row == 1)
        .find_map(|p| {
            map.points()
                .iter()
                .find(|q| {
                    q.coord.row == 2
                        && q.kind == MapPointType::Monster
                        && !p.children.contains(&q.coord)
                })
                .map(|q| (p.coord, q.coord))
        })
        .expect("a monster point off the path");
    run.state.visited = vec![map.start().coord, from];
    run.apply(RunAction::ChooseNode(to))
        .expect("the fight is ported");
    let combat = run.combat().expect("in combat");
    let boots = combat.state().player.relics.last().expect("the boots");
    assert!(matches!(
        boots.data,
        RelicData::WingedBoots(d) if d.times_used == 1
    ));
}

#[test]
fn lucky_fysh_pays_for_every_card_joining_the_deck() {
    // LuckyFysh.cs (AfterCardChangedPiles to the deck), CardPileCmd.cs:540, CardCmd.cs:451.
    let mut run = with_relics(&[RelicKind::LuckyFysh]);
    let s = &mut run.state;
    let gold = s.gold;
    s.add_card(RunCard::new("STRIKE_NECROBINDER"));
    assert_eq!(s.gold, gold + 15);
    let last = s.deck.len() - 1;
    s.transform_to(last, RunCard::new("DEFEND_NECROBINDER"));
    assert_eq!(s.gold, gold + 30);
    // A removal leaves the card with no pile: no gold.
    s.remove_card(last);
    assert_eq!(s.gold, gold + 30);
}

/// `TheScythe.OnPlay` also buffs `DeckVersion`, so a Scythe played in a fight is stronger in
/// every later one (`RL2S55`, a second fight against `FOGMOG`).
#[test]
fn the_scythes_buff_carries_to_the_deck() {
    use crate::combat::Action;
    use crate::content::cards::{CardData, CardKind, the_scythe};

    let mut run = Run::new(
        Character::Necrobinder,
        "RL2S4",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("RL2S4 starts");
    run.apply(RunAction::AncientOption(1))
        .expect("Golden Pearl is ported");
    run.state.deck.push(RunCard::new("THE_SCYTHE"));
    run.apply(RunAction::ChooseNode(MapCoord { col: 1, row: 1 }))
        .expect("the first fight is ported");
    let scythe = run
        .combat()
        .expect("in combat")
        .state()
        .cards
        .iter()
        .find(|c| c.kind == CardKind::TheScythe)
        .expect("the Scythe is in the draw pile")
        .id;
    let mut played = false;
    for _ in 0..200 {
        if run.combat().is_none() {
            break;
        }
        let actions = run.legal_actions();
        let play = actions.iter().find(
            |a| matches!(a, RunAction::Combat(Action::PlayCard { card, .. }) if *card == scythe),
        );
        played |= play.is_some();
        let next = play.or_else(|| actions.first()).expect("an action");
        run.apply(next.clone()).expect("a legal action");
    }
    assert!(played, "the Scythe was played");
    let deck_scythe = run
        .state()
        .deck
        .iter()
        .find(|c| c.id == "THE_SCYTHE")
        .expect("the Scythe is still in the deck");
    assert_eq!(deck_scythe.counter, 5);
    assert_eq!(
        crate::content::cards::data_from_deck(CardKind::TheScythe, deck_scythe.counter),
        CardData::TheScythe(the_scythe::Data {
            increased_damage: 5,
            current_damage: 18
        })
    );
}

#[test]
fn ascension_changes_the_run_start() {
    let elites = |run: &Run| {
        run.state
            .map
            .points()
            .iter()
            .filter(|p| p.kind == MapPointType::Elite)
            .count()
    };
    let a0 = Run::new(
        Character::Necrobinder,
        "RL8",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("run");
    let a10 = Run::new(
        Character::Necrobinder,
        "RL8",
        10,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("run");
    assert_eq!(elites(&a0), 5);
    assert_eq!(elites(&a10), 8);
    assert_eq!(a0.state.potions.len(), 3);
    assert_eq!(a10.state.potions.len(), 2);
    assert!(a0.state.acts[2].second_boss.is_none());
    let second = a10.state.acts[2].second_boss.expect("a second boss");
    assert_ne!(second.id, a10.state.acts[2].boss.id);
    // `RoomSet.NextBossEncounter`: the second boss once a boss of the act was visited.
    let mut glory = a10.state.acts[2].clone();
    assert_eq!(glory.next_boss_encounter().id, glory.boss.id);
    glory.boss_encounters_visited = 1;
    assert_eq!(glory.next_boss_encounter().id, second.id);
    let mut glory_a0 = a0.state.acts[2].clone();
    glory_a0.boss_encounters_visited = 1;
    assert_eq!(glory_a0.next_boss_encounter().id, glory_a0.boss.id);
    assert!(a10.state.card_rarity_odds.scarcity && !a0.state.card_rarity_odds.scarcity);
    assert!(
        Run::new(
            Character::Necrobinder,
            "RL8",
            11,
            UnlockState::all(),
            Act1::Overgrowth
        )
        .is_err()
    );
}

#[test]
fn poverty_truncates_three_quarters() {
    assert_eq!(rewards::poverty_gold(45, 2), 45);
    assert_eq!(rewards::poverty_gold(45, 3), 33);
    assert_eq!(rewards::poverty_gold(35, 3), 26);
    assert_eq!(rewards::poverty_gold(53, 3), 39);
}

#[test]
fn tungsten_rod_reduces_damage_outside_combat() {
    let mut run = Run::new(
        Character::Necrobinder,
        "RL8",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("run");
    let hp = run.state.hp;
    run.state.damage(8);
    assert_eq!(run.state.hp, hp - 8);
    run.state.relics.push(RunRelic::new(RelicKind::TungstenRod));
    run.state.damage(8);
    run.state.damage(1);
    assert_eq!(run.state.hp, hp - 15);
}

#[test]
fn every_character_starts_a_run() {
    // `docs/specs/characters.md` §1-2: HP, gold, energy, deck and starter relic by character.
    for character in Character::ALL {
        let run = Run::new(character, "T6S0", 0, UnlockState::all(), Act1::Overgrowth)
            .expect("the run starts");
        let s = run.state();
        assert_eq!(s.character, character);
        assert_eq!(
            (s.hp, s.max_hp),
            (character.starting_hp(), character.starting_hp())
        );
        assert_eq!((s.gold, s.max_energy), (99, 3));
        let deck: Vec<&str> = s.deck.iter().map(|c| c.id).collect();
        assert_eq!(deck, character.starting_deck());
        assert_eq!(s.relics.len(), 1);
        assert_eq!(s.relics[0].kind, character.starting_relic());
        // The player's bag holds the character's non-starter relics.
        let bag: Vec<&str> = s
            .player_relic_bag
            .deques
            .iter()
            .flat_map(|(_, l)| l.clone())
            .collect();
        for &(id, rarity) in character.relic_pool() {
            assert_eq!(
                bag.contains(&id),
                rarity != relic_bag::RelicRarity::Starter,
                "{id}"
            );
        }
    }
}

#[test]
fn every_starter_deck_fights_and_only_the_regent_starts_with_stars() {
    // DivineRight.cs:16 (3 stars on entering a combat room, C17).
    for c in [
        Character::Ironclad,
        Character::Silent,
        Character::Necrobinder,
        Character::Regent,
    ] {
        let config = CombatConfig::new(c, "D1S0", 0).expect("a ported deck");
        let combat = Combat::new(config);
        let player = &combat.state().player;
        assert_eq!(player.character, c);
        let stars = if c == Character::Regent { 3 } else { 0 };
        assert_eq!((player.orb_slots, player.stars), (0, stars));
        assert_eq!(player.orbs, []);
    }
}

/// `T6S0`'s first fight with a test-only thief (`SwipePower`) holding a stolen deck card; the
/// thief is killed (or escapes) and the fight is won. Returns the deck before the fight and the
/// stolen card's index in it.
fn theft(killed: bool) -> (Run, Vec<RunCard>, usize) {
    use crate::content::powers::test_swipe;
    let mut run = with_relics(&[]);
    let node = run
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, RunAction::ChooseNode(_)))
        .expect("a first node");
    run.apply(node).expect("the first room is a fight");
    let deck = run.state.deck.clone();
    let Phase::Combat(combat) = &mut run.phase else {
        panic!("a fight");
    };
    let deck_index = combat.with_engine(|eng| {
        let card = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .find(|&id| eng.s.card(id).deck_version() == Some(3))
            .expect("deck card 3 is in the draw pile");
        let enemies = eng.s.enemies.clone();
        test_swipe::steal(eng, enemies[0], card)?;
        for e in enemies {
            if killed {
                eng.kill(&[e])?;
            } else {
                eng.escape(e);
            }
        }
        Ok(3)
    });
    run.win_combat().expect("ported");
    (run, deck, deck_index)
}

#[test]
fn a_killed_thief_returns_the_stolen_card_through_a_special_card_reward() {
    // SwipePower.cs:50-78, SpecialCardReward.cs:36,86-96.
    let (mut run, mut deck, i) = theft(true);
    let stolen = deck.remove(i);
    assert_eq!(run.state.deck, deck);
    let RunDecision::Rewards(rewards) = run.pending_decision() else {
        panic!("rewards");
    };
    let special = rewards
        .iter()
        .position(|r| *r == Reward::SpecialCard(stolen))
        .expect("the stolen card is offered back");
    // `RewardsSetIndex` 4 sorts it after gold, potions and relics and before the card reward.
    assert!(matches!(rewards[special + 1], Reward::Card(_)));
    assert!(
        rewards[..special]
            .iter()
            .all(|r| !matches!(r, Reward::Card(_)))
    );
    run.apply(RunAction::ClaimReward(special))
        .expect("a special card reward is claimable");
    deck.push(stolen);
    assert_eq!(run.state.deck, deck);
}

#[test]
fn an_escaped_thief_keeps_the_stolen_card() {
    let (run, mut deck, i) = theft(false);
    deck.remove(i);
    assert_eq!(run.state.deck, deck);
    let RunDecision::Rewards(rewards) = run.pending_decision() else {
        panic!("rewards");
    };
    assert!(!rewards.iter().any(|r| matches!(r, Reward::SpecialCard(_))));
}

/// `FurCoat` marks the first fight's point, so its enemies have 1 HP, and `FestivePopper`'s
/// first-turn damage kills them before the first play: `StartTurn`'s win check
/// (`CombatManager.cs:827`) ends the fight and the room's rewards follow (PX16).
#[test]
fn a_fight_won_during_set_up_goes_to_the_rewards() {
    use crate::content::relics::fur_coat;
    let mut run = with_relics(&[RelicKind::FestivePopper]);
    let to = run
        .legal_actions()
        .into_iter()
        .find_map(|a| match a {
            RunAction::ChooseNode(c)
                if run.state().map.point(c).map(|p| p.kind) == Some(MapPointType::Monster) =>
            {
                Some(c)
            }
            _ => None,
        })
        .expect("a row-1 fight");
    let mut coat = RunRelic::new(RelicKind::FurCoat);
    let mut data = fur_coat::Data {
        fur_coat_act_index: 0,
        len: 1,
        coords_set: true,
        ..fur_coat::Data::default()
    };
    data.cols[0] = u8::try_from(to.col).expect("a column fits u8");
    data.rows[0] = u8::try_from(to.row).expect("a row fits u8");
    coat.data = RelicData::FurCoat(data);
    run.state.relics.push(coat);
    run.apply(RunAction::ChooseNode(to))
        .expect("the first fight is ported");
    assert!(run.combat().is_none(), "the fight is over");
    assert!(matches!(run.pending_decision(), RunDecision::Rewards(_)));
    assert_eq!(run.state().act().normal_encounters_visited, 1);
}
