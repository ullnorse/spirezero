//! Batch XAN2: the Hive ancients `Pael` and `Tezcatara` (option pools and event-`Rng` use, from
//! `Models.Events/Pael.cs:160-181` and `Tezcatara.cs:188-200`), and the run-level effects of
//! their relics (`AfterObtained`, rest-site options, map hooks, `PaelsTooth`'s returns).

use super::*;
use crate::content::ancients::AncientModel;
use crate::content::ancients::pael::Pael;
use crate::content::ancients::tezcatara::Tezcatara;
use crate::content::relics::{golden_compass, paels_eye, pumpkin_candle};
use crate::rng::Rng;
use crate::run::map::ActMap;

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

fn rest_site(run: &mut Run) -> Vec<&'static str> {
    run.state.room = Room::RestSite;
    let options = run.rest_options();
    let ids = options.iter().map(|o| o.kind.id()).collect();
    run.phase = Phase::RestSite(RestSite {
        options,
        used: false,
        pending: None,
    });
    ids
}

fn names(options: &[AncientOption]) -> Vec<&'static str> {
    options.iter().map(|o| o.name()).collect()
}

fn pael(s: &mut RunState, seed: u64) -> (Vec<&'static str>, i32) {
    let mut rng = Rng::new(seed);
    let options = Pael.generate_options(s, &mut rng);
    (names(&options), rng.counter())
}

/// Three `NextItem`s on the lists the decomp builds, in that order.
fn draws(seed: u64, lists: [&[&'static str]; 3]) -> Vec<&'static str> {
    let mut rng = Rng::new(seed);
    lists
        .iter()
        .map(|l| *rng.next_item(l).expect("a pool"))
        .collect()
}

#[test]
fn pael_rolls_its_three_pools() {
    let pool1: &[&str] = &["PAELS_FLESH", "PAELS_HORN", "PAELS_TEARS"];
    // The starter deck: 4 Defends (Goopy), 10 removable cards; no pet. Pool 2 is
    // `[Wing, Claw, Tooth]` doubled, then `Growth`.
    let full: &[&str] = &[
        "PAELS_WING",
        "PAELS_CLAW",
        "PAELS_TOOTH",
        "PAELS_WING",
        "PAELS_CLAW",
        "PAELS_TOOTH",
        "PAELS_GROWTH",
    ];
    let pool3: &[&str] = &["PAELS_EYE", "PAELS_BLOOD", "PAELS_LEGION"];
    let mut run = after_neow();
    for seed in 0..200 {
        let (options, counter) = pael(&mut run.state, seed);
        assert_eq!(counter, 3, "one draw per pool");
        assert_eq!(options, draws(seed, [pool1, full, pool3]));
    }
    // Two Defends, four cards: no Claw, no Tooth. A pet relic drops Legion.
    run.state.deck = [
        "STRIKE_NECROBINDER",
        "STRIKE_NECROBINDER",
        "DEFEND_NECROBINDER",
        "DEFEND_NECROBINDER",
    ]
    .map(RunCard::new)
    .to_vec();
    run.state.relics.push(RunRelic::new(RelicKind::Byrdpip));
    let small: &[&str] = &["PAELS_WING", "PAELS_WING", "PAELS_GROWTH"];
    let no_pet: &[&str] = &["PAELS_EYE", "PAELS_BLOOD"];
    for seed in 0..200 {
        assert_eq!(
            pael(&mut run.state, seed).0,
            draws(seed, [pool1, small, no_pet])
        );
    }
}

#[test]
fn tezcatara_offers_soup_only_with_a_basic_strike() {
    let pool2: &[&str] = &["BIIIG_HUG", "STORYBOOK", "TOASTY_MITTENS"];
    let pool3: &[&str] = &[
        "GOLDEN_COMPASS",
        "PUMPKIN_CANDLE",
        "TOY_BOX",
        "SEAL_OF_GOLD",
    ];
    let mut run = after_neow();
    let with: &[&str] = &["VERY_HOT_COCOA", "YUMMY_COOKIE", "NUTRITIOUS_SOUP"];
    let without: &[&str] = &["VERY_HOT_COCOA", "YUMMY_COOKIE"];
    for seed in 0..200 {
        let mut rng = Rng::new(seed);
        let options = names(&Tezcatara.generate_options(&mut run.state, &mut rng));
        assert_eq!(rng.counter(), 3);
        assert_eq!(options, draws(seed, [with, pool2, pool3]));
    }
    run.state.deck.retain(|c| c.id != "STRIKE_NECROBINDER");
    for seed in 0..200 {
        let options = names(&Tezcatara.generate_options(&mut run.state, &mut Rng::new(seed)));
        assert_eq!(options, draws(seed, [without, pool2, pool3]));
    }
}

#[test]
fn hive_ancient_rooms_roll_on_their_event_rng() {
    for ancient in ["PAEL", "TEZCATARA"] {
        let mut run = after_neow();
        run.enter_next_act().expect("Hive's map generates");
        run.phase = Phase::Map;
        run.state.acts[1].ancient = ancient;
        let start = run.state.map.start().coord;
        run.apply(RunAction::ChooseNode(start)).expect("travelable");
        let RunDecision::Ancient(room) = run.pending_decision() else {
            panic!("{ancient} is ported");
        };
        let mut s = run.state.clone();
        let mut rng = events::event_rng(s.rng.seed(), ancient);
        let expected = room.kind.model().generate_options(&mut s, &mut rng);
        assert_eq!(room.options, expected);
    }
}

#[test]
fn claw_and_soup_enchant_the_starter_cards() {
    let mut run = after_neow();
    obtain(&mut run, "PAELS_CLAW");
    obtain(&mut run, "NUTRITIOUS_SOUP");
    for card in &run.state.deck {
        let kind = card.enchantment.map(|e| (e.kind, e.amount));
        match card.id {
            "DEFEND_NECROBINDER" => assert_eq!(kind, Some((EnchantmentKind::Goopy, 1))),
            "STRIKE_NECROBINDER" => {
                assert_eq!(kind, Some((EnchantmentKind::TezcatarasEmber, 1)));
                // Eternal.
                assert!(!card.is_removable());
            }
            _ => assert_eq!(kind, None),
        }
    }
}

#[test]
fn horn_storybook_cookie_and_hug() {
    let mut run = after_neow();
    let len = run.state.deck.len();
    obtain(&mut run, "PAELS_HORN");
    obtain(&mut run, "STORYBOOK");
    let added: Vec<&str> = run.state.deck[len..].iter().map(|c| c.id).collect();
    assert_eq!(added, ["RELAX", "RELAX", "BRIGHTEST_FLAME"]);
    obtain(&mut run, "YUMMY_COOKIE");
    let Phase::CardSelect(select) = &run.phase else {
        panic!("Yummy Cookie upgrades 4 chosen cards");
    };
    assert_eq!((select.purpose, select.count), (SelectPurpose::Upgrade, 4));
    run.phase = Phase::Map;
    obtain(&mut run, "BIIIG_HUG");
    let Phase::CardSelect(select) = &run.phase else {
        panic!("Biiig Hug removes 4 chosen cards");
    };
    assert_eq!(select.count, 4);
    assert_eq!(select.options, run.state.removable_cards());
}

#[test]
fn growth_clones_its_card_at_rest_sites() {
    let mut run = after_neow();
    obtain(&mut run, "PAELS_GROWTH");
    run.apply(RunAction::SelectCard(9)).expect("a pick");
    let picked = run.state.deck[9];
    assert_eq!(
        picked.enchantment.map(|e| (e.kind, e.amount)),
        Some((EnchantmentKind::Clone, 4))
    );
    assert_eq!(rest_site(&mut run), ["HEAL", "SMITH", "CLONE"]);
    let len = run.state.deck.len();
    run.apply(RunAction::RestOption(2)).expect("clone");
    // `RunState.CloneCard` keeps the enchantment, so the copy clones too.
    assert_eq!(run.state.deck.len(), len + 1);
    assert_eq!(run.state.deck[len], picked);
}

#[test]
fn pumpkin_candle_kindles_on_obtain_and_at_rest_sites() {
    let mut run = after_neow();
    obtain(&mut run, "PUMPKIN_CANDLE");
    let candle = run.state.relics.len() - 1;
    let count = |run: &Run| match run.state.relics[candle].data {
        RelicData::PumpkinCandle(pumpkin_candle::Data { kindle_count }) => kindle_count,
        _ => unreachable!(),
    };
    assert_eq!(count(&run), 5);
    assert_eq!(rest_site(&mut run), ["HEAL", "SMITH", "KINDLE"]);
    run.apply(RunAction::RestOption(2)).expect("kindle");
    assert_eq!(count(&run), 10);
}

#[test]
fn golden_compass_makes_the_current_act_a_golden_path_of_events() {
    let mut run = after_neow();
    obtain(&mut run, "GOLDEN_COMPASS");
    let compass = run.state.relics.last().expect("obtained");
    assert_eq!(
        compass.data,
        RelicData::GoldenCompass(golden_compass::Data { golden_path_act: 0 })
    );
    assert_eq!(run.state.map, ActMap::golden_path());
    let mut types = vec![RoomType::Monster, RoomType::Shop];
    hooks::modify_unknown_map_point_room_types(&run.state, &mut types);
    assert_eq!(types, [RoomType::Event]);
    // Another act keeps its map and `?` odds.
    run.state.act_index = 1;
    let mut types = vec![RoomType::Monster];
    hooks::modify_unknown_map_point_room_types(&run.state, &mut types);
    assert_eq!(types, [RoomType::Monster]);
}

#[test]
fn paels_eye_starts_outside_a_player_turn() {
    let mut run = after_neow();
    obtain(&mut run, "PAELS_EYE");
    assert_eq!(
        run.state.relics.last().map(|r| r.data),
        Some(RelicData::PaelsEye(paels_eye::Data {
            used_this_combat: false,
            was_owner_part_of_last_player_turn: false,
        }))
    );
}

#[test]
fn paels_tooth_keeps_five_cards_and_returns_one_upgraded_per_win() {
    let mut run = after_neow();
    let deck = run.state.deck.clone();
    obtain(&mut run, "PAELS_TOOTH");
    for i in [9, 0, 5, 1, 6] {
        let pick = run.pending_decision().clone();
        let RunDecision::CardSelect(select) = pick else {
            panic!("a removal of 5");
        };
        let at = select
            .options
            .iter()
            .position(|&o| o == i)
            .expect("upgradable");
        run.apply(RunAction::SelectCard(at)).expect("a pick");
    }
    let tooth = run.state.relics.len() - 1;
    let kept = |run: &Run| match run.state.relics[tooth].data {
        RelicData::PaelsTooth(d) => d.cards.iter().flatten().copied().collect::<Vec<_>>(),
        _ => unreachable!(),
    };
    // Ordered by `Id.Entry`.
    let mut expected: Vec<RunCard> = [9, 0, 5, 1, 6].iter().map(|&i| deck[i]).collect();
    expected.sort_by(|a, b| a.id.cmp(b.id));
    assert_eq!(kept(&run), expected);
    assert_eq!(run.state.deck.len(), deck.len() - 5);

    let mut rng = run.state.player_rng[PlayerRngType::Rewards].clone();
    let drawn = *rng.next_item(&expected).expect("a card");
    hooks::after_combat_end_on_deck(&mut run.state, RoomType::Monster);
    let mut back = drawn;
    back.upgrade();
    assert_eq!(run.state.deck.last(), Some(&back));
    assert_eq!(kept(&run).len(), 4);
    assert_eq!(
        run.state.player_rng[PlayerRngType::Rewards].counter(),
        rng.counter()
    );
}

#[test]
fn goopy_amount_carries_to_the_deck() {
    use crate::combat::Action;
    let mut run = after_neow();
    obtain(&mut run, "PAELS_CLAW");
    let first = run
        .state
        .map
        .start()
        .children
        .iter()
        .min()
        .expect("a first room");
    run.apply(RunAction::ChooseNode(first)).expect("a fight");
    let mut plays = 0;
    for _ in 0..300 {
        let Some(combat) = run.combat() else { break };
        let cs = combat.state();
        let actions = run.legal_actions();
        let defend = actions.iter().find(|a| {
            matches!(a, RunAction::Combat(Action::PlayCard { card, .. })
                if cs.card(*card).enchantment.is_some_and(|e| e.kind == EnchantmentKind::Goopy))
        });
        plays += usize::from(defend.is_some());
        let next = defend
            .or_else(|| actions.first())
            .expect("an action")
            .clone();
        run.apply(next).expect("a legal action");
    }
    assert!(plays > 0, "a Goopy card was played");
    // Each play raised its deck version's `Amount` by 1 (Goopy exhausts the card).
    let total: i32 = run
        .state
        .deck
        .iter()
        .filter_map(|c| c.enchantment.filter(|e| e.kind == EnchantmentKind::Goopy))
        .map(|e| e.amount - 1)
        .sum();
    assert_eq!(usize::try_from(total).expect("positive"), plays);
}
