//! Event batch EV4: `FakeMerchant` (`Models.Events/FakeMerchant.cs`), its shop and its fight. The
//! referee cannot enter an event yet (and sts2-cli skips this one), so these pin the inventory, its
//! draws and the options against the decomp.

use super::events::event_rng;
use super::odds::merchant_relic_cost;
use super::*;
use crate::content::characters::Character;
use crate::content::events::EventKind;
use crate::rng::PlayerRngType;
use crate::shuffle::unstable_shuffle;

const FAKES: [&str; 9] = [
    "FAKE_ANCHOR",
    "FAKE_BLOOD_VIAL",
    "FAKE_HAPPY_FLOWER",
    "FAKE_LEES_WAFFLE",
    "FAKE_MANGO",
    "FAKE_ORICHALCUM",
    "FAKE_SNECKO_EYE",
    "FAKE_STRIKE_DUMMY",
    "FAKE_VENERABLE_TEA_SET",
];

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

/// `T6S0` in Act 2 on a map point, with `gold`, standing in `FakeMerchant`.
fn in_fake_merchant(set_up: impl FnOnce(&mut RunState)) -> Run {
    let mut run = t6s0();
    run.state.act_index = 1;
    run.state.visited.push(run.state.map.boss().coord);
    set_up(&mut run.state);
    run.state.room = Room::Event {
        id: EventKind::FakeMerchant.id(),
    };
    run.phase = run
        .begin_event(EventKind::FakeMerchant)
        .expect("the event begins");
    run
}

fn options(run: &Run) -> Vec<(String, bool)> {
    match run.pending_decision() {
        RunDecision::Event(room) => room
            .options
            .iter()
            .map(|o| (o.text_key.clone(), o.action.is_some()))
            .collect(),
        other => panic!("not on an event page: {other:?}"),
    }
}

fn key(name: &str) -> String {
    format!("FAKE_MERCHANT.pages.INITIAL.options.{name}")
}

/// The expected inventory: `UnstableShuffle` on the event `Rng`, the first 6, each `CalcCost` on
/// `Shops` in order.
fn expected_inventory(run: &Run) -> Vec<(&'static str, i32)> {
    let mut rng = event_rng(run.state.rng.seed(), "FAKE_MERCHANT");
    let mut pool = FAKES;
    unstable_shuffle(&mut pool, &mut rng);
    let mut shops = run.state.player_rng[PlayerRngType::Shops].clone();
    pool[..6]
        .iter()
        .map(|&id| (id, merchant_relic_cost(50, &mut shops)))
        .collect()
}

#[test]
fn fake_merchant_needs_act_two_and_gold_or_a_foul_potion() {
    let allowed = |set_up: &dyn Fn(&mut RunState)| {
        let mut run = t6s0();
        set_up(&mut run.state);
        events::is_allowed("FAKE_MERCHANT", &run.state)
    };
    assert!(!allowed(&|s| s.gold = 500));
    assert!(allowed(&|s| {
        s.act_index = 1;
        s.gold = 100;
    }));
    assert!(!allowed(&|s| {
        s.act_index = 1;
        s.gold = 99;
    }));
    assert!(allowed(&|s| {
        s.act_index = 1;
        s.gold = 0;
        s.potions[0] = Some("FOUL_POTION");
    }));
}

#[test]
fn fake_merchant_sells_six_shuffled_relics_priced_on_shops() {
    let before = in_fake_merchant(|s| s.gold = 1000);
    // The inventory is drawn from the streams as they were before the event began.
    let mut fresh = t6s0();
    fresh.state.act_index = 1;
    let inventory = expected_inventory(&fresh);
    let mut want: Vec<_> = inventory.iter().map(|&(id, _)| (key(id), true)).collect();
    want.push((key("LEAVE"), true));
    assert_eq!(options(&before), want);
    let mut shops = fresh.state.player_rng[PlayerRngType::Shops].clone();
    for _ in 0..6 {
        merchant_relic_cost(50, &mut shops);
    }
    assert_eq!(before.state.player_rng[PlayerRngType::Shops], shops);
    for &(_, cost) in &inventory {
        assert!((43..=58).contains(&cost), "50 × [0.85, 1.15): {cost}");
    }
}

#[test]
fn fake_merchant_buys_relics_and_locks_what_it_cannot_afford() {
    let mut fresh = t6s0();
    fresh.state.act_index = 1;
    let inventory = expected_inventory(&fresh);
    let (first, cost) = inventory[0];
    let mut run = in_fake_merchant(|s| s.gold = cost);
    run.apply(RunAction::EventOption(0)).expect("BUY");
    assert_eq!(run.state.gold, 0);
    assert!(run.state.relics.iter().any(|r| r.id() == first));
    // The entry is gone (no restock outside a `MerchantRoom`); with 0 gold the rest are locked.
    let mut want: Vec<_> = inventory[1..]
        .iter()
        .map(|&(id, _)| (key(id), false))
        .collect();
    want.push((key("LEAVE"), true));
    assert_eq!(options(&run), want);
    run.apply(RunAction::EventOption(5)).expect("LEAVE");
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

#[test]
fn fake_merchant_fights_when_a_foul_potion_is_thrown() {
    let mut fresh = t6s0();
    fresh.state.act_index = 1;
    let inventory = expected_inventory(&fresh);
    let mut run = in_fake_merchant(|s| {
        s.gold = 0;
        s.potions[1] = Some("FOUL_POTION");
    });
    let throw = options(&run)
        .iter()
        .position(|(k, _)| *k == key("FOUL_POTION"))
        .expect("a throw option");
    run.apply(RunAction::EventOption(throw))
        .expect("FOUL_POTION");
    assert_eq!(run.state.potions[1], None);
    assert_eq!(
        run.state.room,
        Room::Combat {
            room_type: RoomType::Monster,
            encounter: "FAKE_MERCHANT_EVENT_ENCOUNTER",
        }
    );
    let mut want = vec![PendingReward::Ready(Reward::Relic("FAKE_MERCHANTS_RUG"))];
    want.extend(
        inventory
            .iter()
            .map(|&(id, _)| PendingReward::Ready(Reward::Relic(id))),
    );
    assert_eq!(run.combat_extra_rewards, want);
}

#[test]
fn fake_merchant_fight_is_worth_exactly_300_gold() {
    // `FakeMerchantEventEncounter.MinGoldReward`/`MaxGoldReward` override the room's range and
    // its `Poverty` scaling.
    let mut run = t6s0();
    run.state.ascension = 10;
    run.state.room = Room::Combat {
        room_type: RoomType::Monster,
        encounter: "FAKE_MERCHANT_EVENT_ENCOUNTER",
    };
    let rewards = rewards::generate_combat_rewards(
        &mut run.state,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    assert_eq!(rewards.first(), Some(&Reward::Gold(300)));
}
