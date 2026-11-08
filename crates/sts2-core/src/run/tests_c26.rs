//! C26: core gaps from the sweep (`WarHistorianRepy`, card-reward rerolls and alternatives, wax
//! relics), against the decomp.

use super::*;
use crate::content::characters::Character;
use crate::content::events::EventKind;
use crate::content::relics::{RelicData, RelicKind};
use crate::rng::PlayerRngType;

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

/// `T6S0` in Glory with `keys` Lantern Keys, standing in `WarHistorianRepy`.
fn war_historian(keys: usize) -> Run {
    let mut run = t6s0();
    run.state.act_index = 2;
    for _ in 0..keys {
        run.state.deck.push(RunCard::new("LANTERN_KEY"));
    }
    let kind = EventKind::WarHistorianRepy;
    run.state.room = Room::Event { id: kind.id() };
    run.phase = run.begin_event(kind).expect("the event begins");
    run
}

fn keys(run: &Run) -> usize {
    run.state
        .deck
        .iter()
        .filter(|c| c.id == "LANTERN_KEY")
        .count()
}

fn event_options(run: &Run) -> Vec<String> {
    match run.pending_decision() {
        RunDecision::Event(room) => room.options.iter().map(|o| o.text_key.clone()).collect(),
        other => panic!("not on an event page: {other:?}"),
    }
}

#[test]
fn war_historian_is_never_pulled() {
    let mut run = t6s0();
    run.state.act_index = 2;
    run.state.deck.push(RunCard::new("LANTERN_KEY"));
    assert!(!events::is_allowed("WAR_HISTORIAN_REPY", &run.state));
}

#[test]
fn war_historian_chest_spends_the_key() {
    let mut run = war_historian(1);
    run.apply(RunAction::EventOption(1)).expect("UNLOCK_CHEST");
    assert_eq!(keys(&run), 0);
    let RunDecision::Rewards(rewards) = run.pending_decision() else {
        panic!("the chest offers rewards");
    };
    assert!(matches!(
        rewards,
        [
            Reward::Potion(_),
            Reward::Potion(_),
            Reward::Relic(_),
            Reward::Relic(_)
        ]
    ));
    run.apply(RunAction::Proceed).expect("leave the rewards");
    // Finished: back on the map.
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

#[test]
fn war_historian_second_key_offers_the_other_unlock() {
    let mut run = war_historian(2);
    run.apply(RunAction::EventOption(1)).expect("UNLOCK_CHEST");
    assert_eq!(keys(&run), 1);
    run.apply(RunAction::Proceed).expect("leave the rewards");
    assert_eq!(
        event_options(&run),
        ["WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CAGE"]
    );
    // The second unlock finishes the event and spends every key.
    run.apply(RunAction::EventOption(0))
        .expect("SECOND_UNLOCK_CAGE");
    assert_eq!(keys(&run), 0);
    assert_eq!(
        run.state.relics.last().map(|r| r.kind.id()),
        Some("HISTORY_COURSE")
    );
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

#[test]
fn war_historian_cage_gives_history_course() {
    let mut run = war_historian(1);
    run.apply(RunAction::EventOption(0)).expect("UNLOCK_CAGE");
    assert_eq!(keys(&run), 0);
    assert_eq!(
        run.state.relics.last().map(|r| r.kind.id()),
        Some("HISTORY_COURSE")
    );
}

/// `T6S0` on a won monster fight's rewards screen, holding `relics`.
fn monster_rewards(relics: &[RelicKind]) -> Run {
    let mut run = t6s0();
    run.state
        .relics
        .extend(relics.iter().map(|&k| RunRelic::new(k)));
    run.state.room = Room::Combat {
        room_type: RoomType::Monster,
        encounter: "SHRINKER_BEETLE_WEAK",
    };
    let rewards = rewards::generate_combat_rewards(
        &mut run.state,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    run.phase = Phase::Rewards(rewards);
    run
}

fn card_rewards(run: &Run) -> Vec<(usize, CardReward)> {
    let RunDecision::Rewards(rewards) = run.pending_decision() else {
        panic!("not on a rewards screen");
    };
    rewards
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r {
            Reward::Card(cards) => Some((i, cards.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn card_rewards_offer_skip_only_by_default() {
    let run = monster_rewards(&[]);
    let [(i, ref reward)] = card_rewards(&run)[..] else {
        panic!("one card reward");
    };
    assert!(!reward.can_reroll);
    assert_eq!(run.card_reward_alternatives(i), [CardAlternative::Skip]);
    assert!(
        !run.legal_actions()
            .iter()
            .any(|a| matches!(a, RunAction::CardAlternative { .. }))
    );
}

/// `Driftwood.TryModifyRewardsLate` marks every card reward, `PrayerWheel`'s added one too;
/// `CardReward.Reroll` draws `OptionCount` new cards with the reward's options, once.
#[test]
fn driftwood_rerolls_each_card_reward_once() {
    let mut run = monster_rewards(&[RelicKind::PrayerWheel, RelicKind::Driftwood]);
    let rewards = card_rewards(&run);
    assert_eq!(rewards.len(), 2);
    assert!(rewards.iter().all(|(_, r)| r.can_reroll));
    let (i, before) = rewards[0].clone();
    assert_eq!(
        run.card_reward_alternatives(i),
        [CardAlternative::Skip, CardAlternative::Reroll]
    );
    // The same draws a fresh reward with the same options would make.
    let mut expected = run.state.clone();
    let cards = rewards::create_card_reward(
        &mut expected,
        3,
        before.reroll_options.expect("generated options"),
    );
    run.apply(RunAction::CardAlternative {
        reward: i,
        alternative: 1,
    })
    .expect("REROLL");
    let (_, after) = card_rewards(&run)[0].clone();
    assert_eq!(after.cards, cards);
    assert!(!after.can_reroll);
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        expected.player_rng[PlayerRngType::Rewards].counter()
    );
    assert_eq!(run.card_reward_alternatives(i), [CardAlternative::Skip]);
    run.apply(RunAction::PickCard { reward: i, card: 0 })
        .expect("a rerolled card");
    assert_eq!(run.state.deck.last(), Some(&cards[0]));
}

/// `PaelsWing`'s `SACRIFICE` completes the reward without a card; the 2nd obtains
/// `PullNextRelicFromFront`.
#[test]
fn paels_wing_sacrifices_card_rewards_for_a_relic() {
    let mut run = monster_rewards(&[RelicKind::PrayerWheel, RelicKind::PaelsWing]);
    let [(first, _), (second, _)] = card_rewards(&run)[..] else {
        panic!("two card rewards");
    };
    let wing = run.state.relics.len() - 1;
    let sacrifice = CardAlternative::Relic {
        relic: wing,
        option: "SACRIFICE",
    };
    assert_eq!(
        run.card_reward_alternatives(first),
        [CardAlternative::Skip, sacrifice]
    );
    let deck = run.state.deck.len();
    let relics = run.state.relics.len();
    run.apply(RunAction::CardAlternative {
        reward: first,
        alternative: 1,
    })
    .expect("SACRIFICE");
    assert_eq!(card_rewards(&run).len(), 1);
    assert_eq!(run.state.relics.len(), relics);
    let mut expected = run.state.clone();
    let relic = rewards::pull_relic_from_front(&mut expected);
    run.apply(RunAction::CardAlternative {
        reward: second - 1,
        alternative: 1,
    })
    .expect("SACRIFICE");
    assert_eq!(card_rewards(&run).len(), 0);
    assert_eq!(run.state.deck.len(), deck);
    assert_eq!(run.state.relics.last().map(|r| r.kind.id()), Some(relic));
    assert_eq!(
        run.state.relics[wing].data,
        RelicData::PaelsWing(crate::content::relics::paels_wing::Data {
            rewards_sacrificed: 2
        })
    );
}

/// Skip, `REROLL` and `SACRIFICE` are three alternatives: `CardRewardAlternative.Generate`
/// throws when the reward is opened, so nothing of it can be taken.
#[test]
fn driftwood_and_paels_wing_lock_card_rewards() {
    let mut run = monster_rewards(&[RelicKind::Driftwood, RelicKind::PaelsWing]);
    let [(i, _)] = card_rewards(&run)[..] else {
        panic!("one card reward");
    };
    assert_eq!(run.card_reward_alternatives(i).len(), 3);
    assert!(run.legal_actions().iter().all(|a| !matches!(
        a,
        RunAction::PickCard { .. } | RunAction::CardAlternative { .. }
    )));
    assert_eq!(
        run.apply(RunAction::PickCard { reward: i, card: 0 }),
        Err(RunError::IllegalAction)
    );
    assert!(run.legal_actions().contains(&RunAction::SkipReward(i)));
}

/// `ToyBox.AfterObtained` offers 5 pulled relics as wax `RelicReward`s; claiming one obtains it
/// wax.
#[test]
fn toy_box_offers_five_wax_relics() {
    let mut run = t6s0();
    let mut expected = run.state.clone();
    let pulled: Vec<&str> = (0..5)
        .map(|_| rewards::pull_relic_from_front(&mut expected))
        .collect();
    let phase = run.obtain_relic("TOY_BOX").expect("ported");
    let Some(Phase::Rewards(rewards)) = phase else {
        panic!("a rewards screen");
    };
    assert_eq!(
        rewards,
        pulled
            .iter()
            .map(|&id| Reward::WaxRelic(id))
            .collect::<Vec<_>>()
    );
    run.phase = Phase::Rewards(rewards);
    let relics = run.state.relics.len();
    let claimable = (0..5).find(|&i| {
        let mut next = run.clone();
        next.apply(RunAction::ClaimReward(i)).is_ok()
    });
    let i = claimable.expect("a ported relic among the five");
    run.apply(RunAction::ClaimReward(i)).expect("ported");
    let relic = run.state.relics[relics];
    assert_eq!(relic.id(), pulled[i]);
    assert!(relic.wax && !relic.melted);
}

/// Every 3rd combat melts the first unmelted wax relic: it stays owned but no longer listens
/// (here `Sozu`'s potion veto) and is not tradable; after 15 combats the box is used up.
#[test]
fn toy_box_melts_a_wax_relic_every_third_combat() {
    let mut run = t6s0();
    let wax = |kind| RunRelic {
        wax: true,
        ..RunRelic::new(kind)
    };
    run.state.relics.push(RunRelic::new(RelicKind::ToyBox));
    run.state.relics.push(wax(RelicKind::Anchor));
    run.state.relics.push(wax(RelicKind::Sozu));
    let s = &mut run.state;
    let anchor = s.relics.len() - 2;
    let sozu = anchor + 1;
    assert!(relics::is_tradable(&s.relics[anchor]));
    for _ in 0..2 {
        hooks::after_combat_end_on_deck(s, RoomType::Monster);
    }
    assert!(!s.relics[anchor].melted);
    hooks::after_combat_end_on_deck(s, RoomType::Event);
    assert!(s.relics[anchor].melted && !s.relics[sozu].melted);
    assert!(!relics::is_tradable(&s.relics[anchor]));
    assert!(relics::combat_relics(&s.relics)[anchor].melted);
    assert!(!hooks::should_procure_potion(s));
    for _ in 0..3 {
        hooks::after_combat_end_on_deck(s, RoomType::Elite);
    }
    assert!(s.relics[sozu].melted);
    assert!(hooks::should_procure_potion(s));
    let toy_box = anchor - 1;
    for _ in 0..9 {
        hooks::after_combat_end_on_deck(s, RoomType::Monster);
    }
    assert!(s.relics[toy_box].kind.is_used_up(&s.relics[toy_box].data));
    hooks::after_combat_end_on_deck(s, RoomType::Monster);
    assert_eq!(
        s.relics[toy_box].data,
        RelicData::ToyBox(crate::content::relics::toy_box::Data { combats_seen: 15 })
    );
}

/// `GetDefaultTransformationOptions`: an Event, Ancient or Token card outside a pool transforms
/// into the colorless pool's Common, Uncommon and Rare cards.
#[test]
fn event_and_ancient_cards_transform_into_colorless_cards() {
    let colorless: Vec<&str> = cards::unlocked_colorless_cards()
        .filter(|c| {
            use super::odds::CardRarity::{Common, Rare, Uncommon};
            matches!(c.rarity, Common | Uncommon | Rare)
        })
        .map(|c| c.id)
        .collect();
    for id in ["BYRD_SWOOP", "NEOWS_FURY", "TORIC_TOUGHNESS"] {
        assert_eq!(
            cards::transformation_options(RunCard::new(id)).as_deref(),
            Some(&colorless[..]),
            "{id}"
        );
    }
    assert_eq!(cards::transformation_options(RunCard::new("INJURY")), None);
}

/// `T6S0` on a rewards screen of `rewards` after a fight in `room`.
fn on_rewards(room: RoomType, rewards: Vec<Reward>) -> Run {
    let mut run = t6s0();
    run.state.room = Room::Combat {
        room_type: room,
        encounter: "SHRINKER_BEETLE_WEAK",
    };
    run.phase = Phase::Rewards(rewards);
    run
}

/// `RelicReward.OnSelect` awaits `RelicCmd.Obtain`: `Pomander`'s upgrade selection opens over
/// the rewards screen, which comes back (without the relic) once it is done.
#[test]
fn a_claimed_relic_prompts_over_the_rewards_screen() {
    let mut run = on_rewards(
        RoomType::Monster,
        vec![Reward::Gold(10), Reward::Relic("POMANDER")],
    );
    run.apply(RunAction::ClaimReward(1)).expect("Pomander");
    let RunDecision::CardSelect(select) = run.pending_decision() else {
        panic!("Pomander's selection");
    };
    assert_eq!(select.purpose, SelectPurpose::Upgrade);
    run.apply(RunAction::SelectCard(0)).expect("a card");
    assert_eq!(
        run.pending_decision(),
        RunDecision::Rewards(&[Reward::Gold(10)])
    );
    assert!(run.state.deck.iter().any(|c| c.upgrade_level == 1));
}

/// `SmallCapsule`'s own rewards screen opens over a boss's; leaving it goes back to the boss's
/// screen, not to the next act.
#[test]
fn a_nested_rewards_screen_returns_to_the_boss_rewards() {
    let mut run = on_rewards(
        RoomType::Boss,
        vec![Reward::Gold(100), Reward::Relic("SMALL_CAPSULE")],
    );
    let act = run.state.act_index;
    run.apply(RunAction::ClaimReward(1)).expect("SmallCapsule");
    let RunDecision::Rewards([Reward::Relic(_)]) = run.pending_decision() else {
        panic!("SmallCapsule's relic reward");
    };
    run.apply(RunAction::Proceed)
        .expect("leave the capsule's rewards");
    assert_eq!(
        run.pending_decision(),
        RunDecision::Rewards(&[Reward::Gold(100)])
    );
    assert_eq!(run.state.act_index, act);
}

/// A treasure relic that prompts opens over the chest, which is then empty.
#[test]
fn a_treasure_relic_prompts_over_the_chest() {
    let mut run = t6s0();
    run.state.room = Room::Treasure;
    run.phase = Phase::Treasure(Some("POMANDER"));
    run.apply(RunAction::TakeRelic).expect("Pomander");
    assert!(matches!(run.pending_decision(), RunDecision::CardSelect(_)));
    run.apply(RunAction::SelectCard(0)).expect("a card");
    assert_eq!(run.pending_decision(), RunDecision::Treasure(None));
}
