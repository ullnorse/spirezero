//! Hive event batch EV1: the ten Hive events against the decomp (`Models.Events/<Name>.cs`). The
//! referee cannot enter an event yet, so these pin each option's effect and every draw on the
//! event's `Rng`.

use super::events::event_rng;
use super::*;
use crate::content::cards::CardType;
use crate::content::characters::Character;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::EventKind;

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

/// `T6S0` in Act 2, standing in `kind` (its `IsAllowed` is not checked).
fn in_event(kind: EventKind, set_up: impl FnOnce(&mut RunState)) -> Run {
    let mut run = t6s0();
    run.state.act_index = 1;
    set_up(&mut run.state);
    run.state.room = Room::Event { id: kind.id() };
    run.phase = run.begin_event(kind).expect("the event begins");
    run
}

fn page(run: &Run) -> Vec<String> {
    match run.pending_decision() {
        RunDecision::Event(room) => room.options.iter().map(|o| o.text_key.clone()).collect(),
        other => panic!("not on an event page: {other:?}"),
    }
}

fn select_options(run: &Run) -> Vec<usize> {
    match run.pending_decision() {
        RunDecision::CardSelect(select) => select.options.clone(),
        other => panic!("not on a card selection: {other:?}"),
    }
}

fn on_map(run: &Run) -> bool {
    matches!(run.pending_decision(), RunDecision::Map(_))
}

fn ids(run: &Run) -> Vec<&'static str> {
    run.state.deck.iter().map(|c| c.id).collect()
}

fn allowed(kind: EventKind, set_up: impl FnOnce(&mut RunState)) -> bool {
    let mut run = t6s0();
    set_up(&mut run.state);
    events::is_allowed(kind.id(), &run.state)
}

#[test]
fn amalgamator_combines_two_basic_strikes() {
    assert!(allowed(EventKind::Amalgamator, |_| {}));
    assert!(!allowed(EventKind::Amalgamator, |s| {
        s.deck.retain(|c| !c.is_defend());
        s.deck.push(RunCard::new("DEFEND_NECROBINDER"));
    }));
    let mut run = in_event(EventKind::Amalgamator, |_| {});
    let strikes = run.state.deck.iter().filter(|c| c.is_strike()).count();
    run.apply(RunAction::EventOption(0))
        .expect("COMBINE_STRIKES");
    let options = select_options(&run);
    assert_eq!(options.len(), strikes);
    run.apply(RunAction::SelectCard(0)).expect("a Strike");
    run.apply(RunAction::SelectCard(1)).expect("another Strike");
    let deck = ids(&run);
    assert_eq!(
        deck.iter().filter(|id| id.starts_with("STRIKE_")).count(),
        strikes - 2
    );
    assert_eq!(deck.last(), Some(&"ULTIMATE_STRIKE"));
    assert!(on_map(&run));
}

#[test]
fn bugslayer_adds_its_card() {
    let mut run = in_event(EventKind::Bugslayer, |_| {});
    run.apply(RunAction::EventOption(1)).expect("SQUASH");
    assert_eq!(ids(&run).last(), Some(&"SQUASH"));
    let mut run = in_event(EventKind::Bugslayer, |_| {});
    run.apply(RunAction::EventOption(0)).expect("EXTERMINATION");
    assert_eq!(ids(&run).last(), Some(&"EXTERMINATE"));
}

#[test]
fn colorful_philosophers_drops_one_colour_on_the_event_rng() {
    // The four other colours in `CardPoolColorOrder`, then one `RemoveAt(Rng.NextInt(4))`.
    let run = in_event(EventKind::ColorfulPhilosophers, |_| {});
    let mut rng = event_rng(run.state.rng.seed(), "COLORFUL_PHILOSOPHERS");
    let mut expected = vec!["IRONCLAD", "REGENT", "SILENT", "DEFECT"];
    expected.remove(usize::try_from(rng.next_int(4)).expect("not negative"));
    let keys: Vec<String> = expected
        .iter()
        .map(|c| format!("COLORFUL_PHILOSOPHERS.pages.INITIAL.options.{c}"))
        .collect();
    assert_eq!(page(&run), keys);
    assert_eq!(run.event().map(|e| e.rng.counter()), Some(rng.counter()));
}

#[test]
fn colorful_philosophers_offers_three_card_rewards_of_the_colour() {
    let mut run = in_event(EventKind::ColorfulPhilosophers, |_| {});
    run.apply(RunAction::EventOption(0)).expect("a colour");
    let RunDecision::Rewards(rewards) = run.pending_decision() else {
        panic!("a rewards screen");
    };
    assert_eq!(rewards.len(), 3);
    let character = Character::of_card(match &rewards[0] {
        Reward::Card(cards) => cards.cards[0].id,
        r => panic!("a card reward: {r:?}"),
    })
    .expect("a character card");
    assert_ne!(character, Character::Necrobinder);
    for (reward, rarity) in rewards.iter().zip(["Common", "Uncommon", "Rare"]) {
        let Reward::Card(cards) = reward else {
            panic!("a card reward");
        };
        assert_eq!(cards.cards.len(), 3);
        for card in &cards.cards {
            let info = card.info().expect("a pool card");
            assert_eq!(Character::of_card(card.id), Some(character));
            assert_eq!(format!("{:?}", info.rarity), rarity);
        }
    }
}

#[test]
fn colossal_flower_digs_twice_then_offers_the_core() {
    assert!(!allowed(EventKind::ColossalFlower, |s| s.hp = 18));
    let mut run = in_event(EventKind::ColossalFlower, |s| s.hp = 60);
    assert_eq!(
        page(&run),
        [
            "COLOSSAL_FLOWER.pages.INITIAL.options.EXTRACT_CURRENT_PRIZE_1",
            "COLOSSAL_FLOWER.pages.INITIAL.options.REACH_DEEPER_1",
        ]
    );
    run.apply(RunAction::EventOption(1))
        .expect("REACH_DEEPER_1");
    assert_eq!(run.state.hp, 55);
    assert_eq!(
        page(&run),
        [
            "COLOSSAL_FLOWER.pages.REACH_DEEPER_1.options.EXTRACT_CURRENT_PRIZE_2",
            "COLOSSAL_FLOWER.pages.REACH_DEEPER_1.options.REACH_DEEPER_2",
        ]
    );
    run.apply(RunAction::EventOption(1))
        .expect("REACH_DEEPER_2");
    assert_eq!(run.state.hp, 49);
    assert_eq!(
        page(&run),
        [
            "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.EXTRACT_INSTEAD",
            "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.POLLINOUS_CORE",
        ]
    );
    let mut gold = run.clone();
    let before = gold.state.gold;
    gold.apply(RunAction::EventOption(0))
        .expect("EXTRACT_INSTEAD");
    assert_eq!(gold.state.gold, before + 135);
    run.apply(RunAction::EventOption(1))
        .expect("POLLINOUS_CORE");
    assert_eq!(run.state.hp, 42);
    assert_eq!(
        run.state.relics.last().map(|r| r.kind.id()),
        Some("POLLINOUS_CORE")
    );

    let mut run = in_event(EventKind::ColossalFlower, |_| {});
    let before = run.state.gold;
    run.apply(RunAction::EventOption(1))
        .expect("REACH_DEEPER_1");
    run.apply(RunAction::EventOption(0))
        .expect("EXTRACT_CURRENT_PRIZE_2");
    assert_eq!(run.state.gold, before + 75);
}

#[test]
fn field_of_man_sized_holes_removes_two_or_enchants_one() {
    let mut run = in_event(EventKind::FieldOfManSizedHoles, |_| {});
    let len = run.state.deck.len();
    run.apply(RunAction::EventOption(0)).expect("RESIST");
    run.apply(RunAction::SelectCard(0)).expect("a card");
    run.apply(RunAction::SelectCard(1)).expect("another card");
    assert_eq!(run.state.deck.len(), len - 1);
    assert_eq!(ids(&run).last(), Some(&"NORMALITY"));

    let mut run = in_event(EventKind::FieldOfManSizedHoles, |_| {});
    run.apply(RunAction::EventOption(1))
        .expect("ENTER_YOUR_HOLE");
    let first = select_options(&run)[0];
    run.apply(RunAction::SelectCard(0)).expect("a card");
    let e = run.state.deck[first].enchantment.expect("enchanted");
    assert_eq!((e.kind, e.amount), (EnchantmentKind::PerfectFit, 1));
    assert!(on_map(&run));
}

#[test]
fn infested_automaton_adds_a_power_or_a_zero_cost_card() {
    let mut run = in_event(EventKind::InfestedAutomaton, |_| {});
    run.apply(RunAction::EventOption(0)).expect("STUDY");
    let card = *run.state.deck.last().expect("a card");
    assert_eq!(card.card_type(), Some(CardType::Power));
    let mut run = in_event(EventKind::InfestedAutomaton, |_| {});
    run.apply(RunAction::EventOption(1)).expect("TOUCH_CORE");
    let card = run.state.deck.last().expect("a card").id;
    assert!(Character::Necrobinder.zero_cost_cards().contains(&card));
}

#[test]
fn lost_wisp_rolls_its_gold_on_the_event_rng() {
    // `CalculateVars`: `Gold += Rng.NextInt(-15, 16)`.
    let mut run = in_event(EventKind::LostWisp, |_| {});
    let mut rng = event_rng(run.state.rng.seed(), "LOST_WISP");
    let gold = 60 + rng.next_int_range(-15, 16);
    let before = run.state.gold;
    let mut claim = run.clone();
    run.apply(RunAction::EventOption(1)).expect("SEARCH");
    assert_eq!(run.state.gold, before + gold);
    claim.apply(RunAction::EventOption(0)).expect("CLAIM");
    assert_eq!(ids(&claim).last(), Some(&"DECAY"));
    assert_eq!(
        claim.state.relics.last().map(|r| r.kind.id()),
        Some("LOST_WISP")
    );
}

#[test]
fn spirit_grafter_heals_or_upgrades_and_hurts() {
    let mut run = in_event(EventKind::SpiritGrafter, |s| s.hp = 30);
    run.apply(RunAction::EventOption(0)).expect("LET_IT_IN");
    assert_eq!(run.state.hp, 55);
    assert_eq!(ids(&run).last(), Some(&"METAMORPHOSIS"));
    let mut run = in_event(EventKind::SpiritGrafter, |s| s.hp = 30);
    run.apply(RunAction::EventOption(1)).expect("REJECTION");
    run.apply(RunAction::SelectCard(0)).expect("a card");
    assert_eq!(run.state.deck[0].upgrade_level, 1);
    assert_eq!(run.state.hp, 20);
    assert!(on_map(&run));
}

#[test]
fn the_lantern_key_returns_for_gold_and_its_fight_waits_for_the_knight() {
    let mut run = in_event(EventKind::TheLanternKey, |_| {});
    let before = run.state.gold;
    let mut keep = run.clone();
    run.apply(RunAction::EventOption(0))
        .expect("RETURN_THE_KEY");
    assert_eq!(run.state.gold, before + 100);
    keep.apply(RunAction::EventOption(1)).expect("KEEP_THE_KEY");
    assert_eq!(
        page(&keep),
        ["THE_LANTERN_KEY.pages.KEEP_THE_KEY.options.FIGHT"]
    );
    // `MysteriousKnightEventEncounter` is not ported yet: the option is refused.
    if Encounter::from_entry("MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER").is_none() {
        assert_eq!(
            keep.apply(RunAction::EventOption(0)),
            Err(RunError::UnsupportedEncounter(
                "MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER"
            ))
        );
    }
}

#[test]
fn lantern_key_makes_glory_unknowns_war_historian_events() {
    let mut run = t6s0();
    run.state.deck.push(RunCard::new("LANTERN_KEY"));
    let mut types = vec![RoomType::Monster, RoomType::Event, RoomType::Treasure];
    hooks::modify_unknown_map_point_room_types(&run.state, &mut types);
    assert_eq!(types.len(), 3, "not in Act 1");
    assert_eq!(
        hooks::modify_next_event(&run.state, "BUGSLAYER"),
        "BUGSLAYER"
    );
    run.state.act_index = 2;
    hooks::modify_unknown_map_point_room_types(&run.state, &mut types);
    assert_eq!(types, [RoomType::Event]);
    assert_eq!(
        hooks::modify_next_event(&run.state, "BUGSLAYER"),
        "WAR_HISTORIAN_REPY"
    );
}

#[test]
fn zen_weaver_sells_enlightenment_and_removals() {
    assert!(!allowed(EventKind::ZenWeaver, |s| s.gold = 124));
    let poor = in_event(EventKind::ZenWeaver, |s| s.gold = 130);
    assert_eq!(
        page(&poor),
        [
            "ZEN_WEAVER.pages.INITIAL.options.BREATHING_TECHNIQUES",
            "ZEN_WEAVER.pages.INITIAL.options.EMOTIONAL_AWARENESS",
            "ZEN_WEAVER.pages.INITIAL.options.LOCKED",
        ]
    );
    let mut run = in_event(EventKind::ZenWeaver, |s| s.gold = 300);
    let len = run.state.deck.len();
    let mut breathe = run.clone();
    breathe
        .apply(RunAction::EventOption(0))
        .expect("BREATHING_TECHNIQUES");
    assert_eq!(breathe.state.gold, 250);
    assert_eq!(&ids(&breathe)[len..], ["ENLIGHTENMENT", "ENLIGHTENMENT"]);
    run.apply(RunAction::EventOption(2))
        .expect("ARACHNID_ACUPUNCTURE");
    run.apply(RunAction::SelectCard(0)).expect("a card");
    run.apply(RunAction::SelectCard(1)).expect("another card");
    assert_eq!(run.state.deck.len(), len - 2);
    assert_eq!(run.state.gold, 50);
    assert!(on_map(&run));
}
