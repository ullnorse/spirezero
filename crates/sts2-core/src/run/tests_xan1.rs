//! Batch XAN1: the `Darv` and `Orobas` ancients and their relics, against the decomp
//! (`Models.Events/{Darv,Orobas}.cs`, `Models.Relics/*.cs`). Ancient rooms past Act 1 are not
//! reachable in run diffs yet (C9), so the option rolls and the `AfterObtained` effects are pinned
//! here.

use super::*;
use crate::content::ancients::darv;
use crate::content::cards::CardType;
use crate::content::enchantments::Enchantment;
use crate::content::relics::{archaic_tooth, dusty_tome, sea_glass, touch_of_orobas};
use crate::run::odds::GoldProportion;

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

/// `T6S0` after Neow (Phial Holster), in test mode, in act `act_index`'s state.
fn after_neow(act_index: usize) -> Run {
    let mut run = t6s0().in_test_mode();
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    run.state.act_index = act_index;
    run
}

/// The options `kind` rolls for `s` on its event `Rng`.
fn options(kind: AncientKind, s: &mut RunState) -> Vec<AncientOption> {
    let mut rng = events::event_rng(s.rng.seed(), kind.id());
    kind.model().generate_options(s, &mut rng)
}

fn ids(options: &[AncientOption]) -> Vec<&'static str> {
    options.iter().map(|o| o.name()).collect()
}

/// `Darv.GenerateInitialOptions` replayed by hand on the event `Rng`: one draw per passing set,
/// the shuffle, the `NextBool`.
fn darv_by_hand(s: &RunState, relics: &[&'static str]) -> (Vec<&'static str>, bool) {
    let mut rng = events::event_rng(s.rng.seed(), "DARV");
    let mut list = relics.to_vec();
    for _ in relics {
        assert_eq!(rng.next_int_range(0, 1), 0);
    }
    crate::shuffle::unstable_shuffle(&mut list, &mut rng);
    let tome = rng.next_bool();
    list.truncate(if tome { 2 } else { 3 });
    (list, tome)
}

#[test]
fn darv_offers_the_act_2_sets_shuffled_with_a_possible_dusty_tome() {
    let all: Vec<&str> = darv::all_possible_options().collect();
    assert_eq!(all.len(), 12);
    assert!(
        all.iter()
            .all(|&id| AncientOption::relic(id).is_supported())
    );
    for act_index in [1, 2] {
        let mut run = after_neow(act_index);
        let passing: Vec<&str> = all[..11]
            .iter()
            .copied()
            .filter(|&id| act_index == 1 || !["ECTOPLASM", "SOZU"].contains(&id))
            .collect();
        assert_eq!(passing.len(), if act_index == 1 { 11 } else { 9 });
        let (mut expected, tome) = darv_by_hand(&run.state, &passing);
        let rewards = run.state.player_rng[PlayerRngType::Rewards].clone();
        let got = options(AncientKind::Darv, &mut run.state);
        if tome {
            expected.push("DUSTY_TOME");
            // `SetupForPlayer` draws the card on `rewards`.
            let AncientOption::Relic {
                data: Some(data), ..
            } = got[2]
            else {
                panic!("Dusty Tome is preset");
            };
            let mut s = run.state.clone();
            s.player_rng[PlayerRngType::Rewards] = rewards;
            assert_eq!(dusty_tome::setup_for_player(&mut s), data);
        }
        assert_eq!(ids(&got), expected, "act index {act_index}");
    }
}

#[test]
fn dusty_tome_adds_its_upgraded_ancient_card() {
    let mut run = after_neow(1);
    let before = run.state.player_rng[PlayerRngType::Rewards].counter();
    let data = dusty_tome::setup_for_player(&mut run.state);
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        before + 1
    );
    let RelicData::DustyTome(dusty_tome::Data {
        ancient_card: Some(card),
    }) = data
    else {
        panic!("a card is set up");
    };
    let pool = crate::content::cards::pools::character(Character::Necrobinder);
    let info = pool.iter().find(|c| c.id == card).expect("a pool card");
    assert_eq!(info.rarity, crate::content::cards::CardRarity::Ancient);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_preset_relic("DUSTY_TOME", Some(data))
        .expect("ported");
    assert!(ctx.prompt.is_none());
    let last = *run.state.deck.last().expect("a card");
    assert_eq!((last.id, last.upgrade_level), (card, 1));
}

/// `Orobas.GenerateInitialOptions` replayed by hand for a Necrobinder: the character, the gem
/// roll, then one `NextItem` per pool.
#[test]
fn orobas_rolls_one_option_per_pool() {
    let mut run = after_neow(1);
    let mut rng = events::event_rng(run.state.rng.seed(), "OROBAS");
    let others = [
        Character::Ironclad,
        Character::Silent,
        Character::Regent,
        Character::Defect,
    ];
    let other = *rng.next_item(&others).expect("characters");
    let gem = odds::roll_orobas_prismatic(&mut rng);
    let pool_1 = [
        "ELECTRIC_SHRYMP",
        "GLASS_EYE",
        if gem { "PRISMATIC_GEM" } else { "SEA_GLASS" },
    ];
    let first = *rng.next_item(&pool_1).expect("pool 1");
    let second = *rng
        .next_item(&[
            "ALCHEMICAL_COFFER",
            "DRIFTWOOD",
            "RADIANT_PEARL",
            "SAND_CASTLE",
        ])
        .expect("pool 2");
    let third = *rng
        .next_item(&["TOUCH_OF_OROBAS", "ARCHAIC_TOOTH"])
        .expect("pool 3");
    let got = options(AncientKind::Orobas, &mut run.state);
    assert_eq!(ids(&got), [first, second, third]);
    if first == "SEA_GLASS" {
        assert_eq!(
            got[0],
            AncientOption::Relic {
                id: "SEA_GLASS",
                data: Some(sea_glass::for_character(other)),
            }
        );
    }
    let expected_third = match third {
        "TOUCH_OF_OROBAS" => RelicData::TouchOfOrobas(touch_of_orobas::Data {
            starter_relic: Some("BOUND_PHYLACTERY"),
            upgraded_relic: Some("PHYLACTERY_UNBOUND"),
        }),
        _ => RelicData::ArchaicTooth(archaic_tooth::Data {
            starter_card: Some(RunCard::new("UNLEASH")),
            ancient_card: Some(RunCard::new("PROTECTOR")),
        }),
    };
    assert_eq!(
        got[2],
        AncientOption::Relic {
            id: third,
            data: Some(expected_third),
        }
    );
    assert_eq!(
        got[2].text_key(AncientKind::Orobas),
        format!("OROBAS.pages.INITIAL.options.{third}")
    );
}

#[test]
fn orobas_locks_pool_3_without_a_starter_relic_or_card() {
    let mut run = after_neow(1);
    run.state
        .relics
        .retain(|r| r.kind != RelicKind::BoundPhylactery);
    run.state.deck.retain(|c| c.id != "UNLEASH");
    let got = options(AncientKind::Orobas, &mut run.state);
    assert_eq!(got[2], AncientOption::Locked("OPTION_POOL_3_LOCKED"));
    assert!(!got[2].is_supported());
    assert_eq!(
        got[2].text_key(AncientKind::Orobas),
        "OROBAS.pages.INITIAL.options.OPTION_POOL_3_LOCKED"
    );
    // Any other starter relic upgrades to `Circlet`; a second Touch upgrades the upgrade.
    run.state
        .relics
        .insert(0, RunRelic::new(RelicKind::PhylacteryUnbound));
    assert_eq!(
        touch_of_orobas::setup_for_player(&run.state),
        Some(RelicData::TouchOfOrobas(touch_of_orobas::Data {
            starter_relic: Some("PHYLACTERY_UNBOUND"),
            upgraded_relic: Some("CIRCLET"),
        }))
    );
}

#[test]
fn unlock_state_lists_characters_in_model_db_order() {
    assert_eq!(
        UnlockState::all().characters(),
        Character::MODEL_DB_ORDER.to_vec()
    );
    assert_eq!(UnlockState::none().characters(), [Character::Ironclad]);
}

#[test]
fn touch_of_orobas_replaces_the_starter_in_place() {
    let mut run = after_neow(1);
    run.state.relics.push(RunRelic::new(RelicKind::Anchor));
    let data = touch_of_orobas::setup_for_player(&run.state);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_preset_relic("TOUCH_OF_OROBAS", data)
        .expect("ported");
    let relics: Vec<&str> = run.state.relics.iter().map(|r| r.id()).collect();
    assert_eq!(relics[0], "PHYLACTERY_UNBOUND");
    assert!(!relics.contains(&"BOUND_PHYLACTERY"));
    assert_eq!(relics.last(), Some(&"TOUCH_OF_OROBAS"));
}

#[test]
fn archaic_tooth_transcends_unleash_keeping_upgrade_and_enchantment() {
    let mut run = after_neow(1);
    let i = run
        .state
        .deck
        .iter()
        .position(|c| c.id == "UNLEASH")
        .unwrap();
    run.state.deck[i].upgrade();
    run.state.deck[i].enchantment = Some(Enchantment::new(EnchantmentKind::Sharp, 2));
    let data = archaic_tooth::setup_for_player(&run.state);
    let deck = run.state.deck.len();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_preset_relic("ARCHAIC_TOOTH", data)
        .expect("ported");
    assert_eq!(run.state.deck.len(), deck);
    assert!(run.state.deck.iter().all(|c| c.id != "UNLEASH"));
    let last = *run.state.deck.last().unwrap();
    assert_eq!((last.id, last.upgrade_level), ("PROTECTOR", 1));
    assert_eq!(
        last.enchantment,
        Some(Enchantment::new(EnchantmentKind::Sharp, 2))
    );
}

#[test]
fn astrolabe_transforms_three_picks_into_upgraded_cards() {
    let mut run = after_neow(1);
    let niche = run.state.rng[RunRngType::Niche].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("ASTROLABE").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert_eq!(select.count, 3);
    run.phase = Phase::CardSelect(select);
    let originals: Vec<RunCard> = [0, 4, 9].map(|i| run.state.deck[i]).to_vec();
    for i in [9, 0, 4] {
        run.apply(RunAction::SelectCard(i)).expect("a pick");
    }
    assert_eq!(run.state.rng[RunRngType::Niche].counter(), niche + 3);
    let new: Vec<RunCard> = run.state.deck[run.state.deck.len() - 3..].to_vec();
    assert!(new.iter().all(|c| c.upgrade_level == 1));
    assert!(!run.state.deck.iter().any(|c| c.id == "UNLEASH"));
    assert_eq!(originals[2].id, "UNLEASH");
}

#[test]
fn empty_cage_removes_two_picks() {
    let mut run = after_neow(1);
    let deck = run.state.deck.len();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("EMPTY_CAGE").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert_eq!(select.purpose, SelectPurpose::Remove { damage: 0 });
    assert_eq!(select.count, 2);
    let hp = run.state.hp;
    run.phase = Phase::CardSelect(select);
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    run.apply(RunAction::SelectCard(1)).expect("a pick");
    assert_eq!((run.state.deck.len(), run.state.hp), (deck - 2, hp));
}

#[test]
fn pandoras_box_transforms_every_basic_strike_and_defend() {
    let mut run = after_neow(1);
    let niche = run.state.rng[RunRngType::Niche].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("PANDORAS_BOX").expect("ported");
    assert!(ctx.prompt.is_none());
    assert_eq!(run.state.rng[RunRngType::Niche].counter(), niche + 8);
    assert_eq!(
        &run.state.deck[..2].iter().map(|c| c.id).collect::<Vec<_>>(),
        &["BODYGUARD", "UNLEASH"]
    );
    assert!(
        run.state.deck[2..]
            .iter()
            .all(|c| !c.is_strike() && !c.is_defend())
    );
}

#[test]
fn calling_bell_adds_its_curse_then_offers_three_relics() {
    let mut run = after_neow(1);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("CALLING_BELL").expect("ported");
    let Some(Prompt::Rewards(rewards)) = ctx.prompt else {
        panic!("a rewards screen");
    };
    assert_eq!(
        rewards,
        ["ANCHOR", "GREMLIN_HORN", "MUMMIFIED_HAND"].map(Reward::Relic)
    );
    assert_eq!(run.state.deck.last().unwrap().id, "CURSE_OF_THE_BELL");
    assert!(!run.state.deck.last().unwrap().is_removable());

    // Out of test mode: one relic of each rarity, without a rarity roll.
    let mut run = t6s0();
    run.apply(RunAction::AncientOption(0)).expect("ported");
    let before = run.state.player_rng[PlayerRngType::Rewards].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("CALLING_BELL").expect("ported");
    let Some(Prompt::Rewards(rewards)) = ctx.prompt else {
        panic!("a rewards screen");
    };
    let mut s = run.state.clone();
    let expected: Vec<Reward> = [
        relic_bag::RelicRarity::Common,
        relic_bag::RelicRarity::Uncommon,
        relic_bag::RelicRarity::Rare,
    ]
    .map(|r| Reward::Relic(rewards::pull_relic_of_rarity(&mut s, r, |_| true)))
    .to_vec();
    assert_eq!(rewards.len(), 3);
    assert!(rewards.iter().all(|r| matches!(r, Reward::Relic(_))));
    assert_ne!(rewards, expected, "the relics left the bags");
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        before
    );
}

#[test]
fn electric_shrymp_imbues_one_skill() {
    let mut run = after_neow(1);
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("ELECTRIC_SHRYMP").expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a card selection");
    };
    assert!(
        select
            .options
            .iter()
            .all(|&i| run.state.deck[i].card_type() == Some(CardType::Skill))
    );
    let first = select.options[0];
    run.phase = Phase::CardSelect(select);
    run.apply(RunAction::SelectCard(0)).expect("a pick");
    assert_eq!(
        run.state.deck[first].enchantment,
        Some(Enchantment::new(EnchantmentKind::Imbued, 1))
    );
}

#[test]
fn glass_eye_offers_five_uniform_card_rewards() {
    let mut run = after_neow(1);
    let before = run.state.player_rng[PlayerRngType::Rewards].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("GLASS_EYE").expect("ported");
    let Some(Prompt::Rewards(rewards)) = ctx.prompt else {
        panic!("a rewards screen");
    };
    let rarities: Vec<_> = rewards
        .iter()
        .map(|r| match r {
            Reward::Card(cards) => {
                assert_eq!(cards.cards.len(), 3);
                let rarity = cards.cards[0].info().expect("a pool card").rarity;
                assert!(
                    cards
                        .cards
                        .iter()
                        .all(|c| c.info().unwrap().rarity == rarity)
                );
                rarity
            }
            r => panic!("not a card reward: {r:?}"),
        })
        .collect();
    assert_eq!(
        rarities,
        [
            odds::CardRarity::Common,
            odds::CardRarity::Common,
            odds::CardRarity::Uncommon,
            odds::CardRarity::Uncommon,
            odds::CardRarity::Rare,
        ]
    );
    // Uniform odds and no upgrade roll: one pick per card.
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        before + 15
    );
}

#[test]
fn alchemical_coffer_fills_four_new_slots_unless_sozu() {
    let mut run = after_neow(1);
    let slots = run.state.potions.len();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("ALCHEMICAL_COFFER").expect("ported");
    assert_eq!(run.state.potions.len(), slots + 4);
    assert!(run.state.potions[slots..].iter().all(Option::is_some));

    let mut run = after_neow(1);
    run.state.relics.push(RunRelic::new(RelicKind::Sozu));
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("ALCHEMICAL_COFFER").expect("ported");
    assert!(run.state.potions[slots..].iter().all(Option::is_none));
    assert!(!run.state.procure_potion("FIRE_POTION"));
}

#[test]
fn sand_castle_upgrades_six_random_cards() {
    let mut run = after_neow(1);
    let candidates = run.state.upgradable_cards().len();
    let niche = run.state.rng[RunRngType::Niche].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_relic("SAND_CASTLE").expect("ported");
    let upgraded = run
        .state
        .deck
        .iter()
        .filter(|c| c.upgrade_level == 1)
        .count();
    assert_eq!(upgraded, 6);
    assert_eq!(
        run.state.rng[RunRngType::Niche].counter(),
        niche + i32::try_from(candidates - 1).unwrap()
    );
}

#[test]
fn sea_glass_offers_fifteen_cards_of_its_character_to_pick_from() {
    let mut run = after_neow(1);
    let before = run.state.player_rng[PlayerRngType::Rewards].counter();
    let mut ctx = RunCtx::new(&mut run);
    ctx.obtain_preset_relic(
        "SEA_GLASS",
        Some(sea_glass::for_character(Character::Silent)),
    )
    .expect("ported");
    let Some(Prompt::Select(select)) = ctx.prompt else {
        panic!("a grid selection");
    };
    assert_eq!(select.purpose, SelectPurpose::AddToDeckUpTo);
    assert_eq!(select.offered.len(), 15);
    assert!(
        select
            .offered
            .iter()
            .all(|c| Character::of_card(c.id) == Some(Character::Silent))
    );
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        before + 15
    );
    let picks = [select.offered[3], select.offered[11]];
    let deck = run.state.deck.len();
    run.phase = Phase::CardSelect(select);
    assert!(run.legal_actions().contains(&RunAction::ConfirmSelect));
    run.apply(RunAction::SelectCard(3)).expect("a pick");
    run.apply(RunAction::SelectCard(11)).expect("a pick");
    run.apply(RunAction::ConfirmSelect).expect("confirm");
    assert_eq!(run.state.deck[deck..], picks);
}

#[test]
fn black_star_adds_a_relic_to_elite_rewards_only() {
    let mut run = after_neow(1);
    run.state.relics.push(RunRelic::new(RelicKind::BlackStar));
    let relics = |rewards: &[Reward]| {
        rewards
            .iter()
            .filter(|r| matches!(r, Reward::Relic(_)))
            .count()
    };
    let elite = rewards::generate_combat_rewards(
        &mut run.state,
        RoomType::Elite,
        GoldProportion::ONE,
        Vec::new(),
    );
    assert_eq!(relics(&elite), 2);
    let monster = rewards::generate_combat_rewards(
        &mut run.state,
        RoomType::Monster,
        GoldProportion::ONE,
        Vec::new(),
    );
    assert_eq!(relics(&monster), 0);
}

#[test]
fn ectoplasm_stops_gold_and_prismatic_gem_widens_card_rewards() {
    let mut run = after_neow(1);
    run.state.relics.push(RunRelic::new(RelicKind::Ectoplasm));
    let gold = run.state.gold;
    run.state.gain_gold(Decimal::from(50));
    assert_eq!(run.state.gold, gold);

    let mut run = after_neow(1);
    run.state
        .relics
        .push(RunRelic::new(RelicKind::PrismaticGem));
    let combat = CardCreation::for_combat(odds::CardRarityOddsType::RegularEncounter);
    let options = hooks::modify_card_reward_creation_options(&run.state, combat);
    assert_eq!(options.pool, CardPool::AllCharacters);
    assert_eq!(
        hooks::modify_card_reward_creation_options(&run.state, combat.without_pool_modifications())
            .pool,
        CardPool::Character
    );
    let colorless = CardCreation {
        pool: CardPool::Colorless,
        ..combat
    };
    assert_eq!(
        hooks::modify_card_reward_creation_options(&run.state, colorless).pool,
        CardPool::Colorless
    );
    // Every character's pool, in `ModelDb.AllCharacters` order.
    let possible = rewards::possible_cards(Character::Necrobinder, options);
    let first = Character::of_card(possible[0].id);
    let last = Character::of_card(possible.last().unwrap().id);
    assert_eq!(
        (first, last),
        (Some(Character::Ironclad), Some(Character::Defect))
    );
}

#[test]
fn driftwood_is_obtainable() {
    assert!(AncientOption::relic("DRIFTWOOD").is_supported());
}
