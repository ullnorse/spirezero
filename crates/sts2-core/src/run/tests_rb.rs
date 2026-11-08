//! Batch RB: run-side relics, against the decomp (`Models.Relics/*.cs`). The ones that seldom
//! show up in `tools/diff_run.sh` runs are pinned here.

use super::*;
use crate::content::cards::CardType;
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, girya, lasting_candy, lava_lamp, venerable_tea_set};

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

/// `T6S0` after Neow (Phial Holster) with `kinds` owned, in test mode.
fn with_relics(kinds: &[RelicKind]) -> Run {
    let mut run = t6s0().in_test_mode();
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    run.state
        .relics
        .extend(kinds.iter().map(|&k| RunRelic::new(k)));
    run
}

/// A rest site entered with the run's relics.
fn at_rest_site(run: &mut Run) {
    run.state.room = Room::RestSite;
    let options = run.rest_options();
    run.phase = Phase::RestSite(RestSite {
        options,
        used: false,
        pending: None,
    });
}

fn rest_ids(run: &Run) -> Vec<&'static str> {
    match run.pending_decision() {
        RunDecision::RestSite(options) => options.iter().map(|o| o.kind.id()).collect(),
        d => panic!("not at a rest site: {d:?}"),
    }
}

fn rewards_of(run: &Run) -> &[Reward] {
    match run.pending_decision() {
        RunDecision::Rewards(rewards) => rewards,
        d => panic!("no rewards screen: {d:?}"),
    }
}

/// The relics' ids, in order.
fn relic_ids(run: &Run) -> Vec<&'static str> {
    run.state.relics.iter().map(|r| r.id()).collect()
}

#[test]
fn white_beast_statue_forces_a_potion_in_every_combat_room() {
    let s = with_relics(&[RelicKind::WhiteBeastStatue]).state().clone();
    for room in [RoomType::Monster, RoomType::Elite, RoomType::Boss] {
        assert!(hooks::should_force_potion_reward(&s, room));
    }
    assert!(!hooks::should_force_potion_reward(&s, RoomType::Shop));
    // A forced potion skips the roll, so the odds do not change.
    let mut s = s;
    let odds = s.potion_reward_odds.value;
    let rewards = rewards::generate_combat_rewards(
        &mut s,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    assert!(rewards.iter().any(|r| matches!(r, Reward::Potion(_))));
    assert_eq!(s.potion_reward_odds.value.to_bits(), odds.to_bits());
}

#[test]
fn dingy_rug_adds_the_colorless_pool_to_card_rewards_only() {
    let s = with_relics(&[RelicKind::DingyRug]).state().clone();
    let combat = CardCreation::for_combat(odds::CardRarityOddsType::RegularEncounter);
    assert_eq!(
        hooks::modify_card_reward_creation_options(&s, combat).pool,
        CardPool::CharacterAndColorless
    );
    // `NoCardPoolModifications` and non-reward options are left alone.
    let locked = combat.without_pool_modifications();
    assert_eq!(
        hooks::modify_card_reward_creation_options(&s, locked).pool,
        CardPool::Character
    );
    let plain = CardCreation::non_combat_default(CardPool::Character);
    assert_eq!(
        hooks::modify_card_reward_creation_options(&s, plain).pool,
        CardPool::Character
    );
    // The union lists the character cards first, then the colorless ones.
    let pool = CardPool::CharacterAndColorless.union_colorless();
    assert_eq!(pool, CardPool::CharacterAndColorless);
    let possible = rewards::possible_cards(Character::Necrobinder, CardCreation { pool, ..combat });
    let characters = rewards::possible_cards(Character::Necrobinder, combat).len();
    assert!(possible.len() > characters);
    let ids = |cards: &[&cards::PoolCard]| cards.iter().map(|c| c.id).collect::<Vec<_>>();
    assert_eq!(
        ids(&possible[..characters]),
        ids(&rewards::possible_cards(Character::Necrobinder, combat))
    );
}

#[test]
fn eggs_upgrade_their_card_type_in_rewards_the_deck_and_the_shop() {
    let mut run = with_relics(&[
        RelicKind::MoltenEgg,
        RelicKind::ToxicEgg,
        RelicKind::FrozenEgg,
    ]);
    // `TryModifyCardBeingAddedToDeck`: Attack, Skill and Power each upgrade once.
    for (id, upgraded) in [
        ("STRIKE_NECROBINDER", true),
        ("DEFEND_NECROBINDER", true),
        ("CALCIFY", true),
        ("DEBT", false),
    ] {
        let before = run.state.deck.len();
        run.state.add_card(RunCard::new(id));
        assert_eq!(run.state.deck.len(), before + 1);
        let card = run.state.deck[before];
        assert_eq!(card.upgrade_level, u8::from(upgraded), "{id}");
    }
    // `TryModifyCardRewardOptionsLate` upgrades every card of the types, unless
    // `NoHookUpgrades`.
    let mut s = run.state.clone();
    let mut cards = vec![
        RunCard::new("STRIKE_NECROBINDER"),
        RunCard::new("DEFEND_NECROBINDER"),
        RunCard::new("CALCIFY"),
    ];
    let options = CardCreation::for_combat(odds::CardRarityOddsType::RegularEncounter);
    hooks::modify_card_reward_options(&mut s, &mut cards, options);
    assert!(cards.iter().all(|c| c.upgrade_level == 1));
    let mut cards = vec![RunCard::new("STRIKE_NECROBINDER")];
    let locked = CardCreation {
        flags: CreationFlags::NO_HOOK_UPGRADES,
        ..options
    };
    hooks::modify_card_reward_options(&mut s, &mut cards, locked);
    assert_eq!(cards[0].upgrade_level, 0);
    // `ModifyMerchantCardCreationResults`: every stocked card of the types comes upgraded.
    let mut run = with_relics(&[RelicKind::MoltenEgg]);
    run.state.gold = 1000;
    run.state.room = Room::Shop;
    let shop = Shop::generate(&mut run.state);
    for entry in &shop.cards {
        let card = entry.card.expect("stocked");
        let attack = card.card_type() == Some(CardType::Attack);
        assert_eq!(card.upgrade_level, u8::from(attack), "{}", card.id);
    }
}

#[test]
fn molten_egg_claimed_from_the_rewards_screen_upgrades_the_card_reward() {
    // `CardReward.OnRelicObtained`.
    let mut run = with_relics(&[]);
    run.state.room = Room::Combat {
        room_type: RoomType::Elite,
        encounter: "PHROG_PARASITE_ELITE",
    };
    run.phase = Phase::Rewards(vec![
        Reward::Relic("MOLTEN_EGG"),
        Reward::Card(CardReward::with_cards(
            vec![
                RunCard::new("STRIKE_NECROBINDER"),
                RunCard::new("DEFEND_NECROBINDER"),
            ],
            CardCreation::for_combat(odds::CardRarityOddsType::EliteEncounter),
        )),
    ]);
    run.apply(RunAction::ClaimReward(0)).expect("ported");
    let Reward::Card(cards) = &rewards_of(&run)[0] else {
        panic!("the card reward stays");
    };
    assert_eq!(
        cards
            .cards
            .iter()
            .map(|c| c.upgrade_level)
            .collect::<Vec<_>>(),
        [1, 0]
    );
}

#[test]
fn lava_lamp_upgrades_the_rewards_of_a_fight_without_damage() {
    let mut run = with_relics(&[RelicKind::LavaLamp]);
    run.state.room = Room::Combat {
        room_type: RoomType::Monster,
        encounter: "SLIMES_WEAK",
    };
    let me = run.state.relics.len() - 1;
    let options = CardCreation::for_combat(odds::CardRarityOddsType::RegularEncounter);
    let mut cards = vec![
        RunCard::new("STRIKE_NECROBINDER"),
        RunCard::new("DEFEND_NECROBINDER"),
    ];
    hooks::modify_card_reward_options(&mut run.state, &mut cards, options);
    assert!(cards.iter().all(|c| c.upgrade_level == 1));
    // A fight that cost HP leaves them alone.
    run.state.relics[me].data = RelicData::LavaLamp(lava_lamp::Data {
        took_damage_this_combat: true,
    });
    let mut cards = vec![RunCard::new("STRIKE_NECROBINDER")];
    hooks::modify_card_reward_options(&mut run.state, &mut cards, options);
    assert_eq!(cards[0].upgrade_level, 0);
    // Entering a room clears the flag; outside a combat room it never upgrades.
    hooks::after_room_entered(&mut run.state, RoomType::Shop);
    run.state.room = Room::Shop;
    let mut cards = vec![RunCard::new("STRIKE_NECROBINDER")];
    hooks::modify_card_reward_options(&mut run.state, &mut cards, options);
    assert_eq!(cards[0].upgrade_level, 0);
    assert_eq!(
        run.state.relics[me].data,
        RelicData::LavaLamp(lava_lamp::Data::default())
    );
}

#[test]
fn lasting_candy_adds_a_power_to_every_second_combat_card_reward() {
    let mut s = with_relics(&[RelicKind::LastingCandy]).state().clone();
    s.room = Room::Combat {
        room_type: RoomType::Monster,
        encounter: "SLIMES_WEAK",
    };
    let sizes: Vec<usize> = (0..4)
        .map(|_| {
            let rewards = rewards::generate_combat_rewards(
                &mut s,
                RoomType::Monster,
                odds::GoldProportion::ONE,
                Vec::new(),
            );
            let Some(Reward::Card(cards)) = rewards.iter().find(|r| matches!(r, Reward::Card(_)))
            else {
                panic!("a card reward");
            };
            if cards.cards.len() == 4 {
                assert_eq!(cards.cards[3].card_type(), Some(CardType::Power));
            }
            cards.cards.len()
        })
        .collect();
    // The first reward it sees only counts; the second is the triggering one.
    assert_eq!(sizes, [3, 4, 3, 4]);
    let me = s.relics.len() - 1;
    assert_eq!(
        s.relics[me].data,
        RelicData::LastingCandy(lasting_candy::Data {
            combat_rewards_seen: 4
        })
    );
}

#[test]
fn girya_lifts_three_times_and_adds_strength_to_fights() {
    let mut run = with_relics(&[RelicKind::Girya]);
    for lifts in 1..=3 {
        at_rest_site(&mut run);
        assert_eq!(rest_ids(&run), ["HEAL", "SMITH", "LIFT"]);
        let lift = rest_ids(&run).iter().position(|&id| id == "LIFT").unwrap();
        run.apply(RunAction::RestOption(lift)).expect("lift");
        assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
        let girya = run.state.relics.last().unwrap();
        assert_eq!(
            girya.data,
            RelicData::Girya(girya::Data {
                times_lifted: lifts
            })
        );
    }
    at_rest_site(&mut run);
    assert_eq!(rest_ids(&run), ["HEAL", "SMITH"]);
    // `AfterRoomEntered`: Strength equal to the lifts at the start of a fight.
    let first = run
        .state
        .map
        .start()
        .children
        .iter()
        .min()
        .expect("a first room");
    run.state.room = Room::Map;
    run.state.visited.truncate(1);
    run.phase = Phase::Map;
    run.apply(RunAction::ChooseNode(first)).expect("a fight");
    let cs = run.combat().expect("in combat").state();
    let player = cs.creature(cs.player.creature);
    assert_eq!(player.power_amount(PowerKind::Strength), 3);
}

#[test]
fn shovel_digs_a_relic_and_the_tent_keeps_the_rest_site_open() {
    let mut run = with_relics(&[RelicKind::Shovel]);
    at_rest_site(&mut run);
    assert_eq!(rest_ids(&run), ["HEAL", "SMITH", "DIG"]);
    let mut expected = run.state.clone();
    let id = rewards::pull_relic_from_front(&mut expected);
    run.apply(RunAction::RestOption(2)).expect("dig");
    assert_eq!(relic_ids(&run).last(), Some(&id));
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        expected.player_rng[PlayerRngType::Rewards].counter()
    );
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    // `MiniatureTent`: the other options stay after one is used.
    let mut run = with_relics(&[RelicKind::MiniatureTent]);
    run.state.hp = 10;
    at_rest_site(&mut run);
    run.apply(RunAction::RestOption(0)).expect("heal");
    assert_eq!(rest_ids(&run), ["SMITH"]);
    assert!(run.legal_actions().contains(&RunAction::Proceed));
    assert_eq!(run.state.hp, 10 + 19);
}

#[test]
fn tiny_mailbox_offers_two_potions_with_a_rest_site_heal() {
    let mut run = with_relics(&[RelicKind::TinyMailbox]);
    run.state.potions = vec![None; 3];
    at_rest_site(&mut run);
    let rewards = run.state.player_rng[PlayerRngType::Rewards].counter();
    run.apply(RunAction::RestOption(0)).expect("heal");
    assert!(
        rewards_of(&run)
            .iter()
            .all(|r| matches!(r, Reward::Potion(_)))
    );
    assert_eq!(rewards_of(&run).len(), 2);
    assert!(run.state.player_rng[PlayerRngType::Rewards].counter() > rewards);
    run.apply(RunAction::ClaimReward(0)).expect("a free slot");
    run.apply(RunAction::Proceed).expect("leave");
    // Smith is still unused afterwards only with the tent; here the site closes.
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

#[test]
fn dream_catcher_offers_a_three_card_reward_with_a_rest_site_heal() {
    let mut run = with_relics(&[RelicKind::DreamCatcher]);
    run.state.hp = 20;
    at_rest_site(&mut run);
    run.apply(RunAction::RestOption(0)).expect("heal");
    let [Reward::Card(cards)] = rewards_of(&run) else {
        panic!("expected one card reward: {:?}", rewards_of(&run));
    };
    assert_eq!(cards.cards.len(), 3);
}

#[test]
fn planisphere_heals_on_unknown_points_only() {
    let mut run = with_relics(&[RelicKind::Planisphere]);
    run.state.hp = 20;
    let entry = |t| MapPointHistoryEntry {
        point_type: t,
        room_type: Some(RoomType::Event),
        model: None,
    };
    run.state.history.push(entry(MapPointType::Monster));
    hooks::after_room_entered(&mut run.state, RoomType::Shop);
    assert_eq!(run.state.hp, 20);
    run.state.history.push(entry(MapPointType::Unknown));
    hooks::after_room_entered(&mut run.state, RoomType::Event);
    assert_eq!(run.state.hp, 25);
    // A `?` that became a fight heals before it starts.
    hooks::after_map_combat_room_entered(&mut run.state, MapPointType::Unknown);
    assert_eq!(run.state.hp, 30);
    hooks::after_map_combat_room_entered(&mut run.state, MapPointType::Monster);
    assert_eq!(run.state.hp, 30);
}

#[test]
fn venerable_tea_set_gives_energy_on_the_first_turn_after_a_rest_site() {
    let mut run = with_relics(&[RelicKind::VenerableTeaSet]);
    let first = run.state.map.start().children.first().expect("a child");
    hooks::after_room_entered(&mut run.state, RoomType::RestSite);
    run.apply(RunAction::ChooseNode(first)).expect("a fight");
    let cs = run.combat().expect("in combat").state();
    assert_eq!(cs.player.energy, 3 + 2);
    // Used up: the next fight starts with the usual energy.
    run.leave_combat();
    let tea = run.state.relics.last().unwrap();
    assert_eq!(
        tea.data,
        RelicData::VenerableTeaSet(venerable_tea_set::Data::default())
    );
}

#[test]
fn cauldron_offers_five_potions_fixed_in_test_mode() {
    let mut run = with_relics(&[]);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("CAULDRON").expect("ported");
    let Some(Prompt::Rewards(rewards)) = ctx.prompt else {
        panic!("a rewards screen");
    };
    assert_eq!(
        rewards,
        [
            Reward::Potion("FLEX_POTION"),
            Reward::Potion("WEAK_POTION"),
            Reward::Potion("VULNERABLE_POTION"),
            Reward::Potion("STRENGTH_POTION"),
            Reward::Potion("DEXTERITY_POTION"),
        ]
    );
    // Off test mode each is a `PotionReward(Owner)` rolled on `rewards`.
    let mut run = t6s0();
    run.apply(RunAction::AncientOption(0)).expect("ported");
    run.state.test_mode = false;
    let before = run.state.player_rng[PlayerRngType::Rewards].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("CAULDRON").expect("ported");
    let Some(Prompt::Rewards(rewards)) = ctx.prompt else {
        panic!("a rewards screen");
    };
    assert_eq!(rewards.len(), 5);
    assert!(rewards.iter().all(|r| matches!(r, Reward::Potion(_))));
    assert!(run.state.player_rng[PlayerRngType::Rewards].counter() >= before + 5);
}

#[test]
fn orrery_offers_five_card_rewards_of_three() {
    let mut run = with_relics(&[]);
    let before = run.state.player_rng[PlayerRngType::Rewards].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("ORRERY").expect("ported");
    let Some(Prompt::Rewards(rewards)) = ctx.prompt else {
        panic!("a rewards screen");
    };
    assert_eq!(rewards.len(), 5);
    assert!(
        rewards
            .iter()
            .all(|r| matches!(r, Reward::Card(cards) if cards.cards.len() == 3))
    );
    // Per card: the rarity roll, the pick and the upgrade roll.
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        before + 5 * 3 * 3
    );
}

#[test]
fn war_paint_and_whetstone_upgrade_two_random_cards_of_their_type() {
    for (kind, id, card_type) in [
        (RelicKind::WarPaint, "WAR_PAINT", CardType::Skill),
        (RelicKind::Whetstone, "WHETSTONE", CardType::Attack),
    ] {
        let mut run = with_relics(&[]);
        let candidates = run
            .state
            .deck
            .iter()
            .filter(|c| c.card_type() == Some(card_type) && c.is_upgradable())
            .count();
        let before = run.state.rng[RunRngType::Niche].counter();
        let mut ctx = RunCtx::new(&mut run);
        ctx.obtain_relic(id).expect("ported");
        assert!(ctx.prompt.is_none());
        let upgraded: Vec<RunCard> = run
            .state
            .deck
            .iter()
            .copied()
            .filter(|c| c.upgrade_level > 0)
            .collect();
        assert_eq!(upgraded.len(), 2, "{kind:?}");
        assert!(upgraded.iter().all(|c| c.card_type() == Some(card_type)));
        // `StableShuffle` draws once per element but the first.
        assert_eq!(
            run.state.rng[RunRngType::Niche].counter(),
            before + i32::try_from(candidates - 1).unwrap()
        );
    }
}

#[test]
fn dollys_mirror_copies_the_picked_card() {
    let mut run = with_relics(&[]);
    run.state.deck[3].upgrade();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("DOLLYS_MIRROR").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert_eq!(
        select.options,
        (0..run.state.deck.len()).collect::<Vec<_>>()
    );
    run.phase = Phase::CardSelect(select);
    let picked = run.state.deck[3];
    run.apply(RunAction::SelectCard(3)).expect("a pick");
    assert_eq!(run.state.deck.last(), Some(&picked));
    assert_eq!(picked.upgrade_level, 1);
}

#[test]
fn gnarled_hammer_enchants_up_to_three_cards_with_sharp() {
    let mut run = with_relics(&[]);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("GNARLED_HAMMER").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    // Sharp enchants Attacks only.
    let attacks: Vec<usize> = (0..run.state.deck.len())
        .filter(|&i| run.state.deck[i].card_type() == Some(CardType::Attack))
        .collect();
    assert_eq!(select.options, attacks);
    assert_eq!((select.min(), select.count), (0, 3));
    run.phase = Phase::CardSelect(select);
    // Nothing is picked yet, and the player may still confirm with none.
    assert!(run.legal_actions().contains(&RunAction::ConfirmSelect));
    run.apply(RunAction::SelectCard(1)).expect("a pick");
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    run.apply(RunAction::ConfirmSelect).expect("confirm");
    let enchanted: Vec<usize> = (0..run.state.deck.len())
        .filter(|&i| run.state.deck[i].enchantment.is_some())
        .collect();
    assert_eq!(enchanted, [attacks[0], attacks[1]]);
    let sharp = run.state.deck[attacks[0]].enchantment.unwrap();
    assert_eq!((sharp.kind, sharp.amount), (EnchantmentKind::Sharp, 3));
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

#[test]
fn sword_of_stone_becomes_sword_of_jade_after_five_elites() {
    let mut run = with_relics(&[RelicKind::Girya, RelicKind::SwordOfStone]);
    let me = run.state.relics.len() - 1;
    for fought in 1..=5 {
        run.after_combat_victory(RoomType::Monster).expect("ported");
        run.after_combat_victory(RoomType::Elite).expect("ported");
        let kinds: Vec<RelicKind> = run.state.relics.iter().map(|r| r.kind).collect();
        if fought < 5 {
            assert_eq!(kinds[me], RelicKind::SwordOfStone);
        } else {
            // `RelicCmd.Replace`: Jade takes the Stone's place.
            assert_eq!(kinds[me], RelicKind::SwordOfJade);
            assert_eq!(kinds[me - 1], RelicKind::Girya);
        }
    }
}

#[test]
fn neows_bones_offers_two_neow_relics_then_adds_a_curse() {
    // T6S0's third Neow option.
    let mut run = t6s0().in_test_mode();
    assert_eq!(
        run.ancient_relics(),
        ["PHIAL_HOLSTER", "BOOMING_CONCH", "NEOWS_BONES"]
    );
    // The two relics come from `Neow.AllPossibleOptions` (minus itself and the multiplayer
    // relic) shuffled on `rewards`.
    let mut expected: Vec<&str> = crate::content::ancients::neow::all_possible_options()
        .filter(|&id| id != "NEOWS_BONES" && id != "MASSIVE_SCROLL")
        .collect();
    assert_eq!(expected.len(), 28);
    crate::shuffle::unstable_shuffle(
        &mut expected,
        &mut run.state.player_rng[PlayerRngType::Rewards].clone(),
    );
    let niche = run.state.rng[RunRngType::Niche].counter();
    let deck = run.state.deck.len();
    run.apply(RunAction::AncientOption(2))
        .expect("Neow's Bones is ported");
    assert_eq!(
        rewards_of(&run),
        [Reward::Relic(expected[0]), Reward::Relic(expected[1])]
    );
    // The curse follows the screen, not the pick.
    assert_eq!(run.state.deck.len(), deck);
    run.phase = Phase::Rewards(Vec::new());
    run.apply(RunAction::Proceed).expect("leave the screen");
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
    assert_eq!(run.state.deck.len(), deck + 1);
    assert_eq!(run.state.rng[RunRngType::Niche].counter(), niche + 1);
    let curse = run.state.deck.last().unwrap().id;
    assert!(
        [
            "CLUMSY",
            "DEBT",
            "DECAY",
            "DOUBT",
            "GUILTY",
            "INJURY",
            "NORMALITY",
            "REGRET",
            "SHAME",
            "WRITHE"
        ]
        .contains(&curse)
    );
}

/// The deck indices an enchantment can take in the starter deck.
fn enchantable(run: &Run, kind: EnchantmentKind) -> Vec<usize> {
    run.state.enchantable_cards(kind).expect("a ported deck")
}

#[test]
fn kifuda_enchants_up_to_three_cards_with_adroit() {
    let mut run = with_relics(&[]);
    let options = enchantable(&run, EnchantmentKind::Adroit);
    assert!(options.len() > 1);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("KIFUDA").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert_eq!(select.options, options);
    assert_eq!((select.min(), select.count), (0, 3));
    run.phase = Phase::CardSelect(select);
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    run.apply(RunAction::ConfirmSelect).expect("confirm");
    let enchantment = run.state.deck[options[0]].enchantment.expect("enchanted");
    assert_eq!(
        (enchantment.kind, enchantment.amount),
        (EnchantmentKind::Adroit, 3)
    );
    assert_eq!(
        run.state
            .deck
            .iter()
            .filter(|c| c.enchantment.is_some())
            .count(),
        1
    );
}

#[test]
fn punch_dagger_enchants_one_card_with_momentum() {
    let mut run = with_relics(&[]);
    let options = enchantable(&run, EnchantmentKind::Momentum);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("PUNCH_DAGGER").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert_eq!(select.options, options);
    assert_eq!((select.min(), select.count), (1, 1));
    run.phase = Phase::CardSelect(select);
    run.apply(RunAction::SelectCard(1)).expect("a pick");
    let enchantment = run.state.deck[options[1]].enchantment.expect("enchanted");
    assert_eq!(
        (enchantment.kind, enchantment.amount),
        (EnchantmentKind::Momentum, 5)
    );
}

#[test]
fn royal_stamp_shuffles_on_niche_but_lists_the_deck_in_order() {
    let mut run = with_relics(&[]);
    let options = enchantable(&run, EnchantmentKind::RoyallyApproved);
    let niche = run.state.rng[RunRngType::Niche].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("ROYAL_STAMP").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert_eq!(select.options, options);
    // `UnstableShuffle` draws once per element but the first.
    assert_eq!(
        run.state.rng[RunRngType::Niche].counter(),
        niche + i32::try_from(options.len() - 1).unwrap()
    );
    run.phase = Phase::CardSelect(select);
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    let enchantment = run.state.deck[options[0]].enchantment.expect("enchanted");
    assert_eq!(enchantment.kind, EnchantmentKind::RoyallyApproved);
}

#[test]
fn wing_charm_enchants_one_card_of_each_reward_with_swift() {
    let mut s = with_relics(&[RelicKind::WingCharm]).state().clone();
    let niche = s.rng[RunRngType::Niche].counter();
    let rewards = rewards::generate_combat_rewards(
        &mut s,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    let Some(Reward::Card(cards)) = rewards.iter().find(|r| matches!(r, Reward::Card(_))) else {
        panic!("a card reward");
    };
    let swift: Vec<&RunCard> = cards
        .cards
        .iter()
        .filter(|c| {
            c.enchantment
                .is_some_and(|e| e.kind == EnchantmentKind::Swift)
        })
        .collect();
    assert_eq!(swift.len(), 1);
    assert_eq!(swift[0].enchantment.map(|e| e.amount), Some(1));
    // One `NextItem` on `Niche`.
    assert_eq!(s.rng[RunRngType::Niche].counter(), niche + 1);
}
