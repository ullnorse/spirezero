//! Shared event batch EV3: `CrystalSphere`, `DollRoom`, `PotionCourier`, `RanwidTheElder`,
//! `RelicTrader`, `StoneOfAllTime`, `Symbiote`, `WelcomeToWongos` and their run-side relics,
//! against the decomp (`Models.Events/<Name>.cs`). The referee cannot enter an event yet, so these
//! pin each option's effect and every draw on the event's `Rng`.

use super::events::{EventAction, event_rng};
use super::*;
use crate::content::characters::Character;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::crystal_sphere::{ItemKind, Minigame};
use crate::content::events::{EventData, EventKind};
use crate::content::relics::{bing_bong, wongos_mystery_ticket};
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

/// `T6S0` in Act 2 with 300 gold, standing in `kind` (its `IsAllowed` is not checked).
fn in_event(kind: EventKind, set_up: impl FnOnce(&mut RunState)) -> Run {
    let mut run = t6s0();
    run.state.act_index = 1;
    run.state.gold = 300;
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

fn rewards(run: &Run) -> Vec<Reward> {
    match run.pending_decision() {
        RunDecision::Rewards(rewards) => rewards.to_vec(),
        other => panic!("not on a rewards screen: {other:?}"),
    }
}

fn on_map(run: &Run) -> bool {
    matches!(run.pending_decision(), RunDecision::Map(_))
}

fn last_relic(run: &Run) -> Option<&'static str> {
    run.state.relics.last().map(|r| r.kind.id())
}

/// `IsAllowed` in act `act_index` (0-based) with `set_up` applied.
fn allowed(kind: EventKind, act_index: usize, set_up: impl FnOnce(&mut RunState)) -> bool {
    let mut run = t6s0();
    run.state.act_index = act_index;
    set_up(&mut run.state);
    events::is_allowed(kind.id(), &run.state)
}

fn relics(s: &mut RunState, kinds: &[RelicKind]) {
    s.relics.extend(kinds.iter().map(|&k| RunRelic::new(k)));
}

/// Five pool relics of tradable rarities.
const TRADABLE: [RelicKind; 5] = [
    RelicKind::Akabeko,
    RelicKind::Anchor,
    RelicKind::BagOfMarbles,
    RelicKind::Lantern,
    RelicKind::Vajra,
];

#[test]
fn act_gates_refuse_act_one() {
    let rich = |s: &mut RunState| {
        s.gold = 500;
        s.potions[0] = Some("FIRE_POTION");
        relics(s, &TRADABLE);
    };
    // (event, allowed in Act 1, 2, 3)
    let gates = [
        (EventKind::CrystalSphere, [false, true, true]),
        (EventKind::DollRoom, [false, true, false]),
        (EventKind::PotionCourier, [false, true, true]),
        (EventKind::RanwidTheElder, [false, true, true]),
        (EventKind::RelicTrader, [false, true, true]),
        (EventKind::StoneOfAllTime, [false, true, false]),
        (EventKind::Symbiote, [false, true, true]),
        (EventKind::WelcomeToWongos, [false, true, false]),
    ];
    for (kind, expected) in gates {
        for (act, &allowed_in_act) in expected.iter().enumerate() {
            assert_eq!(
                allowed(kind, act, rich),
                allowed_in_act,
                "{kind:?} act {act}"
            );
        }
    }
    // The other conditions, in Act 2.
    assert!(!allowed(EventKind::CrystalSphere, 1, |s| s.gold = 99));
    assert!(!allowed(EventKind::WelcomeToWongos, 1, |s| s.gold = 99));
    assert!(!allowed(EventKind::StoneOfAllTime, 1, |s| s.potions = vec![None; 3]));
    assert!(!allowed(EventKind::RanwidTheElder, 1, |s| {
        rich(s);
        s.potions = vec![None; 3];
    }));
    assert!(!allowed(EventKind::RanwidTheElder, 1, |s| {
        rich(s);
        s.gold = 99;
    }));
    // The starter relic is not tradable; neither is an Event relic or a used-up one.
    assert!(!allowed(EventKind::RanwidTheElder, 1, |s| {
        s.gold = 500;
        s.potions[0] = Some("FIRE_POTION");
        relics(s, &[RelicKind::BingBong, RelicKind::Strawberry]);
    }));
    assert!(!allowed(EventKind::RelicTrader, 1, |s| relics(
        s,
        &TRADABLE[..4]
    )));
}

#[test]
fn tradable_relics_follow_is_tradable() {
    use super::relics::is_tradable;
    assert!(
        !is_tradable(&RunRelic::new(RelicKind::BoundPhylactery)),
        "Starter"
    );
    assert!(!is_tradable(&RunRelic::new(RelicKind::BingBong)), "Event");
    assert!(
        !is_tradable(&RunRelic::new(RelicKind::Strawberry)),
        "upon pickup"
    );
    assert!(is_tradable(&RunRelic::new(RelicKind::Anchor)));
    let mut tail = RunRelic::new(RelicKind::LizardTail);
    assert!(is_tradable(&tail));
    let RelicData::LizardTail(data) = &mut tail.data else {
        panic!("LizardTail data");
    };
    data.was_used = true;
    assert!(!is_tradable(&tail), "used up");
}

#[test]
fn crystal_sphere_rolls_its_cost_then_places_fifteen_items() {
    let run = in_event(EventKind::CrystalSphere, |_| {});
    let mut rng = event_rng(run.state.rng.seed(), "CRYSTAL_SPHERE");
    let cost = 50 + rng.next_int_range(1, 50);
    assert_eq!(
        run.event().map(|e| e.data.clone()),
        Some(EventData::Amount(cost.into()))
    );
    let mut paid = run;
    paid.apply(RunAction::EventOption(0))
        .expect("UNCOVER_FUTURE");
    assert_eq!(paid.state.gold, 300 - cost);
    let game = Minigame::new(&mut rng, 3);
    // One `NextItem` per item, none failed.
    assert_eq!(rng.counter(), 1 + 15);
    assert_eq!(game.items.len(), 15);
    assert!(game.items.iter().all(|i| i.subscriptions == 1));
    // 6 cells cleared around each corner; one big and one small option per hidden cell.
    let hidden = game.hidden.iter().filter(|&&h| h).count();
    assert_eq!(hidden, 121 - 24);
    assert_eq!(page(&paid).len(), 2 * hidden);
    let event = paid.event().expect("in the event");
    assert_eq!(event.data, EventData::CrystalSphere(Box::new(game)));
    assert_eq!(event.rng.counter(), rng.counter());
}

/// The minigame of the run's event.
fn minigame(run: &Run) -> Minigame {
    match run.event().map(|e| e.data.clone()) {
        Some(EventData::CrystalSphere(game)) => *game,
        other => panic!("no minigame: {other:?}"),
    }
}

/// Clicks cell `index` with the small tool.
fn click_small(run: &mut Run, index: usize) {
    click(run, "SMALL", index);
}

/// Clicks cell `index` with the `tool`.
fn click(run: &mut Run, tool: &'static str, index: usize) {
    let RunDecision::Event(room) = run.pending_decision() else {
        panic!("on the minigame");
    };
    let option = room
        .options
        .iter()
        .position(|o| {
            o.action
                == Some(EventAction {
                    tag: tool,
                    arg: index,
                })
        })
        .expect("a hidden cell");
    run.apply(RunAction::EventOption(option))
        .expect("a divination");
}

/// The cells of the `n`th item of `kind`.
fn item_cells(game: &Minigame, kind: ItemKind, n: usize) -> Vec<usize> {
    let index = (0..game.items.len())
        .filter(|&i| game.items[i].kind == kind)
        .nth(n)
        .expect("the item");
    (0..game.cell_items.len())
        .filter(|&c| game.cell_items[c] == Some(index))
        .collect()
}

#[test]
fn crystal_sphere_small_gold_is_ten_gold_each() {
    let mut run = in_event(EventKind::CrystalSphere, |_| {});
    run.apply(RunAction::EventOption(0))
        .expect("UNCOVER_FUTURE");
    let game = minigame(&run);
    let rng_before = run.event().map(|e| e.rng.counter());
    for n in 0..3 {
        let cells = item_cells(&game, ItemKind::Gold { big: false }, n);
        assert_eq!(cells.len(), 1);
        click_small(&mut run, cells[0]);
    }
    // Gold rewards draw nothing.
    assert_eq!(rewards(&run), vec![Reward::Gold(10); 3]);
    assert_eq!(run.event().map(|e| e.rng.counter()), rng_before);
}

#[test]
fn crystal_sphere_payment_plan_reveals_a_curse_and_a_card_reward() {
    let mut run = in_event(EventKind::CrystalSphere, |_| {});
    let gold = run.state.gold;
    run.apply(RunAction::EventOption(1)).expect("PAYMENT_PLAN");
    assert_eq!(run.state.gold, gold);
    assert_eq!(run.state.deck.last().map(|c| c.id), Some("DEBT"));
    let game = minigame(&run);
    let curse = item_cells(&game, ItemKind::Curse, 0);
    assert_eq!(curse.len(), 4);
    for &c in &curse[..3] {
        click_small(&mut run, c);
    }
    assert_eq!(run.state.deck.last().map(|c| c.id), Some("DEBT"));
    click_small(&mut run, curse[3]);
    assert_eq!(run.state.deck.last().map(|c| c.id), Some("DOUBT"));
    // The common card reward: its picks and upgrade rolls are on the event `Rng`.
    let cards = item_cells(&game, ItemKind::CardReward(CardRarity::Common), 0);
    let player_rewards = run.state.player_rng[PlayerRngType::Rewards].counter();
    let event_draws = run.event().map_or(0, |e| e.rng.counter());
    // The big tool on the card's top-left cell clears all four of its cells.
    click(&mut run, "BIG", cards[0]);
    let game = minigame(&run);
    let spare = (0..game.hidden.len())
        .find(|&c| game.hidden[c] && game.cell_items[c].is_none())
        .expect("an empty hidden cell");
    click_small(&mut run, spare);
    let offered = rewards(&run);
    let card_rewards: Vec<&Vec<RunCard>> = offered
        .iter()
        .filter_map(|r| match r {
            Reward::Card(cards) => Some(&cards.cards),
            _ => None,
        })
        .collect();
    let [cards] = card_rewards[..] else {
        panic!("one card reward: {offered:?}");
    };
    assert_eq!(cards.len(), 3);
    assert!(cards.iter().all(|c| {
        Character::of_card(c.id) == Some(Character::Necrobinder)
            && c.info().is_some_and(|i| i.rarity == CardRarity::Common)
    }));
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        player_rewards
    );
    // At least the three `NextItem`s (and the upgrade rolls) drew on the event `Rng`.
    assert!(run.event().map_or(0, |e| e.rng.counter()) >= event_draws + 3);
}

#[test]
fn doll_room_random_takes_examine_offers_all_three() {
    let mut run = in_event(EventKind::DollRoom, |_| {});
    let mut rng = event_rng(run.state.rng.seed(), "DOLL_ROOM");
    let dolls = ["DAUGHTER_OF_THE_WIND", "MR_STRUGGLES", "BING_BONG"];
    let expected = *rng.next_item(&dolls).expect("a doll");
    let mut random = run.clone();
    random.apply(RunAction::EventOption(0)).expect("RANDOM");
    assert_eq!(last_relic(&random), Some(expected));
    assert!(on_map(&random));

    let hp = run.state.hp;
    let mut take = run.clone();
    take.apply(RunAction::EventOption(1))
        .expect("TAKE_SOME_TIME");
    assert_eq!(take.state.hp, hp - 5);
    assert_eq!(page(&take).len(), 2);

    run.apply(RunAction::EventOption(2)).expect("EXAMINE");
    assert_eq!(run.state.hp, hp - 15);
    // `StableShuffle`: sorted by id (BING_BONG, DAUGHTER_OF_THE_WIND, MR_STRUGGLES), shuffled.
    let mut rng = event_rng(run.state.rng.seed(), "DOLL_ROOM");
    let mut sorted = ["Bing Bong", "Daughter of the Wind", "Mr. Struggles"];
    crate::shuffle::unstable_shuffle(&mut sorted, &mut rng);
    assert_eq!(page(&run), sorted);
    let first = RelicKind::ALL
        .into_iter()
        .find(|k| k.name() == sorted[0])
        .expect("a doll");
    run.apply(RunAction::EventOption(0)).expect("a doll");
    assert_eq!(last_relic(&run), Some(first.id()));
}

#[test]
fn potion_courier_offers_foul_potions_or_an_uncommon() {
    let mut run = in_event(EventKind::PotionCourier, |_| {});
    let mut grab = run.clone();
    grab.apply(RunAction::EventOption(0)).expect("GRAB_POTIONS");
    assert_eq!(rewards(&grab), vec![Reward::Potion("FOUL_POTION"); 3]);
    run.apply(RunAction::EventOption(1)).expect("RANSACK");
    let [Reward::Potion(potion)] = rewards(&run)[..] else {
        panic!("one potion");
    };
    assert_eq!(
        potions::potion_rarity(potion),
        Some(odds::PotionRarity::Uncommon)
    );
}

#[test]
fn foul_potion_is_thrown_only_at_a_merchant() {
    let mut run = in_event(EventKind::PotionCourier, |s| {
        s.potions[0] = Some("FOUL_POTION");
    });
    assert!(!run.legal_actions().contains(&RunAction::UsePotion(0)));
    run.phase = Phase::Map;
    run.state.room = Room::Shop;
    let gold = run.state.gold;
    assert!(run.potion_actions().contains(&RunAction::UsePotion(0)));
    run.apply(RunAction::UsePotion(0)).expect("thrown");
    assert_eq!(run.state.gold, gold + 100);
}

#[test]
fn ranwid_trades_a_potion_gold_or_a_relic() {
    let set_up = |s: &mut RunState| {
        s.potions[0] = Some("FIRE_POTION");
        s.potions[2] = Some("BLOCK_POTION");
        relics(
            s,
            &[RelicKind::BingBong, RelicKind::Anchor, RelicKind::Vajra],
        );
    };
    let run = in_event(EventKind::RanwidTheElder, set_up);
    let mut rng = event_rng(run.state.rng.seed(), "RANWID_THE_ELDER");
    let slot = *rng.next_item(&[0, 2]).expect("a potion");
    let relic = *rng.next_item(&[2, 3]).expect("a tradable relic");
    let RunDecision::Event(room) = run.pending_decision() else {
        panic!("on the event");
    };
    assert_eq!(room.options[0].action.map(|a| a.arg), Some(slot));
    assert_eq!(room.options[2].action.map(|a| a.arg), Some(relic));
    // The belt is locked while the event runs.
    assert_eq!(run.discard_actions(), []);

    let relics_before = run.state.relics.len();
    let mut potion = run.clone();
    potion.apply(RunAction::EventOption(0)).expect("POTION");
    assert_eq!(potion.state.potions[slot], None);
    assert_eq!(potion.state.relics.len(), relics_before + 1);
    assert!(on_map(&potion));
    assert!(
        !potion.discard_actions().is_empty(),
        "unlocked once it is over"
    );

    let mut gold = run.clone();
    gold.apply(RunAction::EventOption(1)).expect("GOLD");
    assert_eq!(gold.state.gold, 200);
    assert_eq!(gold.state.relics.len(), relics_before + 1);

    let mut trade = run;
    let given = trade.state.relics[relic].kind;
    trade.apply(RunAction::EventOption(2)).expect("RELIC");
    assert_eq!(trade.state.relics.len(), relics_before + 1);
    assert!(trade.state.relics.iter().all(|r| r.kind != given));
}

#[test]
fn relic_trader_swaps_one_of_three() {
    let run = in_event(EventKind::RelicTrader, |s| relics(s, &TRADABLE));
    assert_eq!(
        page(&run),
        [
            "RELIC_TRADER.pages.INITIAL.options.TOP",
            "RELIC_TRADER.pages.INITIAL.options.MIDDLE",
            "RELIC_TRADER.pages.INITIAL.options.BOTTOM",
        ]
    );
    let Some(EventData::RelicTrader { owned, new }) = run.event().map(|e| e.data.clone()) else {
        panic!("the trades");
    };
    // `StableShuffle` of the tradable relics (indices 1-5, already in id order) on the event `Rng`.
    let mut rng = event_rng(run.state.rng.seed(), "RELIC_TRADER");
    let mut expected = vec![1, 2, 3, 4, 5];
    crate::shuffle::unstable_shuffle(&mut expected, &mut rng);
    assert_eq!(owned, expected[..3]);
    assert_eq!(new.len(), 3);
    let mut run = run;
    let given = run.state.relics[owned[1]].kind;
    run.apply(RunAction::EventOption(1)).expect("MIDDLE");
    assert!(run.state.relics.iter().all(|r| r.kind != given));
    assert_eq!(last_relic(&run), Some(new[1]));
    assert!(on_map(&run));
}

#[test]
fn stone_of_all_time_lifts_a_potion_or_pushes_for_vigorous() {
    let set_up = |s: &mut RunState| {
        s.potions[1] = Some("FIRE_POTION");
    };
    let run = in_event(EventKind::StoneOfAllTime, set_up);
    assert_eq!(
        page(&run),
        [
            "STONE_OF_ALL_TIME.pages.INITIAL.options.LIFT",
            "STONE_OF_ALL_TIME.pages.INITIAL.options.PUSH",
        ]
    );
    assert!(run.discard_actions().is_empty(), "the belt is locked");
    let mut lift = run.clone();
    let max_hp = lift.state.max_hp;
    lift.apply(RunAction::EventOption(0)).expect("LIFT");
    assert_eq!(lift.state.potions[1], None);
    assert_eq!(lift.state.max_hp, max_hp + 10);
    assert!(on_map(&lift));
    assert!(
        !lift
            .discard_actions()
            .contains(&RunAction::DiscardPotion(1))
    );

    let mut push = run;
    let hp = push.state.hp;
    push.apply(RunAction::EventOption(1)).expect("PUSH");
    assert_eq!(push.state.hp, hp - 6);
    push.apply(RunAction::SelectCard(0)).expect("an attack");
    let enchanted: Vec<_> = push
        .state
        .deck
        .iter()
        .filter_map(|c| c.enchantment)
        .collect();
    assert_eq!(enchanted.len(), 1);
    assert_eq!(enchanted[0].kind, EnchantmentKind::Vigorous);
    assert_eq!(enchanted[0].amount, 8);
    // `NextItem` of one potion draws nothing; each option ends with `NextInt(100)`.
    assert!(on_map(&push));
}

#[test]
fn stone_of_all_time_draws_on_its_rng() {
    let run = in_event(EventKind::StoneOfAllTime, |s| {
        s.potions[0] = Some("FIRE_POTION");
        s.potions[1] = Some("BLOCK_POTION");
    });
    let mut rng = event_rng(run.state.rng.seed(), "STONE_OF_ALL_TIME");
    let slot = *rng.next_item(&[0, 1]).expect("a potion");
    let RunDecision::Event(room) = run.pending_decision() else {
        panic!("on the event");
    };
    assert_eq!(room.options[0].action.map(|a| a.arg), Some(slot));
    assert_eq!(run.event().map(|e| e.rng.counter()), Some(rng.counter()));
}

#[test]
fn symbiote_corrupts_an_attack_or_transforms() {
    let run = in_event(EventKind::Symbiote, |_| {});
    assert_eq!(
        page(&run),
        [
            "SYMBIOTE.pages.INITIAL.options.APPROACH",
            "SYMBIOTE.pages.INITIAL.options.KILL_WITH_FIRE",
        ]
    );
    let mut approach = run.clone();
    approach.apply(RunAction::EventOption(0)).expect("APPROACH");
    approach.apply(RunAction::SelectCard(0)).expect("an attack");
    let corrupted: Vec<_> = approach
        .state
        .deck
        .iter()
        .filter(|c| {
            c.enchantment
                .is_some_and(|e| e.kind == EnchantmentKind::Corrupted)
        })
        .collect();
    assert_eq!(corrupted.len(), 1);
    assert_eq!(
        corrupted[0].card_type(),
        Some(crate::content::cards::CardType::Attack)
    );

    let mut fire = run;
    let len = fire.state.deck.len();
    fire.apply(RunAction::EventOption(1))
        .expect("KILL_WITH_FIRE");
    fire.apply(RunAction::SelectCard(0)).expect("a card");
    assert_eq!(fire.state.deck.len(), len);
    assert!(on_map(&fire));

    let locked = in_event(EventKind::Symbiote, |s| s.deck.retain(|c| !c.is_strike()));
    let attacks = locked
        .state
        .deck
        .iter()
        .any(|c| c.card_type() == Some(crate::content::cards::CardType::Attack));
    if !attacks {
        assert_eq!(
            page(&locked)[0],
            "SYMBIOTE.pages.INITIAL.options.APPROACH_LOCKED"
        );
    }
}

#[test]
fn welcome_to_wongos_sells_relics_by_gold() {
    let poor = in_event(EventKind::WelcomeToWongos, |s| s.gold = 150);
    assert_eq!(
        page(&poor),
        [
            "WELCOME_TO_WONGOS.pages.INITIAL.options.BARGAIN_BIN",
            "WELCOME_TO_WONGOS.pages.INITIAL.options.FEATURED_ITEM_LOCKED",
            "WELCOME_TO_WONGOS.pages.INITIAL.options.MYSTERY_BOX_LOCKED",
            "WELCOME_TO_WONGOS.pages.INITIAL.options.LEAVE",
        ]
    );
    let run = in_event(EventKind::WelcomeToWongos, |_| {});
    let Some(EventData::Relic(featured)) = run.event().map(|e| e.data.clone()) else {
        panic!("the featured relic");
    };
    // The featured Rare is pulled from the front of the player's bag, on entering.
    assert!(
        !run.state
            .player_relic_bag
            .deques
            .iter()
            .any(|(_, l)| l.contains(&featured))
    );

    let mut bargain = run.clone();
    bargain
        .apply(RunAction::EventOption(0))
        .expect("BARGAIN_BIN");
    assert_eq!(bargain.state.gold, 200);
    assert!(bargain.state.relics.len() > run.state.relics.len());

    let mut buy = run.clone();
    buy.apply(RunAction::EventOption(1)).expect("FEATURED_ITEM");
    assert_eq!(buy.state.gold, 100);
    assert_eq!(last_relic(&buy), Some(featured));

    let mut mystery = run;
    mystery
        .apply(RunAction::EventOption(2))
        .expect("MYSTERY_BOX");
    assert_eq!(mystery.state.gold, 0);
    assert_eq!(last_relic(&mystery), Some("WONGOS_MYSTERY_TICKET"));

    let mut leave = in_event(EventKind::WelcomeToWongos, |s| {
        s.deck[0].upgrade_level = 1;
        s.deck[3].upgrade_level = 1;
    });
    let mut rng = event_rng(leave.state.rng.seed(), "WELCOME_TO_WONGOS");
    let downgraded = *rng.next_item(&[0, 3]).expect("an upgraded card");
    leave.apply(RunAction::EventOption(3)).expect("LEAVE");
    assert_eq!(leave.state.deck[downgraded].upgrade_level, 0);
    assert_eq!(leave.state.deck[3 - downgraded].upgrade_level, 1);
}

#[test]
fn bing_bong_clones_every_card_joining_the_deck() {
    let mut run = t6s0();
    relics(&mut run.state, &[RelicKind::BingBong, RelicKind::LuckyFysh]);
    let len = run.state.deck.len();
    let gold = run.state.gold;
    let mut card = RunCard::new("DEFEND_NECROBINDER");
    card.upgrade_level = 1;
    run.state.add_card(card);
    assert_eq!(run.state.deck.len(), len + 2);
    assert_eq!(run.state.deck[len..], [card, card]);
    // Lucky Fysh sees the clone join too.
    assert_eq!(run.state.gold, gold + 30);
    assert_eq!(
        run.state.relics[1].data,
        RelicData::BingBong(bing_bong::Data {
            cards_to_skip: 1,
            adding_clone: false,
        })
    );
}

#[test]
fn wongos_mystery_ticket_gives_three_relics_after_five_combats() {
    let mut run = t6s0();
    relics(&mut run.state, &[RelicKind::WongosMysteryTicket]);
    let me = run.state.relics.len() - 1;
    let ticket = |combats_finished, gave_relic| {
        RelicData::WongosMysteryTicket(wongos_mystery_ticket::Data {
            combats_finished,
            gave_relic,
        })
    };
    run.state.relics[me].data = ticket(4, false);
    let before = rewards::generate_combat_rewards(
        &mut run.state.clone(),
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    assert!(!before.iter().any(|r| matches!(r, Reward::Relic(_))));
    run.state.relics[me].data = ticket(5, false);
    let after = rewards::generate_combat_rewards(
        &mut run.state,
        RoomType::Monster,
        odds::GoldProportion::ONE,
        Vec::new(),
    );
    assert_eq!(
        after
            .iter()
            .filter(|r| matches!(r, Reward::Relic(_)))
            .count(),
        3
    );
    assert_eq!(run.state.relics[me].data, ticket(5, true));
    assert!(RelicKind::WongosMysteryTicket.is_used_up(&run.state.relics[me].data));
}
