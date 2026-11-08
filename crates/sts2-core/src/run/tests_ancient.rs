//! C8: the ancient room at the start of acts 2 and 3 (`AncientEventModel`, `ActModel.PullAncient`),
//! with the test-only `content/ancients/test_ancient.rs` standing in for the act's ancient.

use super::*;
use crate::content::relics::lava_rock;

/// `T6S0` at `ascension` after Neow (Phial Holster), moved to Act 2's map screen as
/// `EnterNextAct` leaves it, with `ancient` as Act 2's ancient and `hp` HP.
fn in_act2(ascension: u8, ancient: &'static str, hp: i32) -> Run {
    let mut run = Run::new(
        Character::Necrobinder,
        "T6S0",
        ascension,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("T6S0 starts");
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    run.enter_next_act().expect("Hive's map generates");
    run.state.acts[1].ancient = ancient;
    run.state.hp = hp;
    run
}

const TEST_ANCIENT: &str = "TEST_ANCIENT";

/// The options `TestAncient` rolls on the event `Rng` of `T6S0` (`runSeed + xxh64("TEST_ANCIENT")`).
fn expected_options(s: &RunState) -> Vec<AncientOption> {
    AncientKind::TestAncient.model().generate_options(
        &mut s.clone(),
        &mut events::event_rng(s.rng.seed(), TEST_ANCIENT),
    )
}

#[test]
fn act_2_starts_on_its_ancient() {
    let mut run = in_act2(0, TEST_ANCIENT, 20);
    let start = run.state.map.start();
    assert_eq!(start.kind, MapPointType::Ancient);
    let start = start.coord;
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(smallvec::smallvec![start])
    );
    let (floor, visited) = (run.state.total_floor(), run.state.act().events_visited);
    run.apply(RunAction::ChooseNode(start))
        .expect("the start is travelable");

    let s = run.state();
    // Not Neow: the missing HP is healed, from the current HP.
    assert_eq!(s.hp, s.max_hp);
    assert_eq!(s.room, Room::Ancient { id: TEST_ANCIENT });
    assert_eq!(
        s.history.last(),
        Some(&MapPointHistoryEntry {
            point_type: MapPointType::Ancient,
            room_type: Some(RoomType::Event),
            model: Some(TEST_ANCIENT),
        })
    );
    assert_eq!(s.total_floor(), floor + 1);
    // `MarkRoomVisited(Event)`; `PullAncient` adds no visited event.
    assert_eq!(s.act().events_visited, visited + 1);
    assert!(!s.visited_event_ids.contains(&TEST_ANCIENT));
    let RunDecision::Ancient(room) = run.pending_decision() else {
        panic!("not on the ancient's page");
    };
    assert_eq!(room.kind, AncientKind::TestAncient);
    // The options are rolled after the heal on the ancient's own event `Rng`.
    assert_eq!(room.options, expected_options(s));
    assert_eq!(room.options.len(), 3);
    assert_eq!(
        room.options[0].text_key(room.kind),
        format!(
            "TEST_ANCIENT.pages.INITIAL.options.{}",
            room.options[0].name()
        )
    );

    let relic = room.options[0]
        .relic_id()
        .expect("the first pool holds relics");
    let relics = run.state.relics.len();
    run.apply(RunAction::AncientOption(0))
        .expect("the relic is ported");
    assert_eq!(run.state.relics.len(), relics + 1);
    assert_eq!(run.state.relics[relics].kind.id(), relic);
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(run.state.map.start().children.iter().collect())
    );
}

#[test]
fn weary_traveler_heals_80_percent_of_the_missing_hp() {
    let mut run = in_act2(2, TEST_ANCIENT, 20);
    let start = run.state.map.start().coord;
    let max = run.state.max_hp;
    run.apply(RunAction::ChooseNode(start)).expect("travelable");
    // `CreatureCmd.Heal(missing × 0.8m)` truncates.
    let missing = Decimal::from(max - 20) * Decimal::new(8, 1);
    assert_eq!(run.state.hp, 20 + missing.to_int_truncating());
}

/// A preset relic keeps its fields; a locked option is shown but cannot be chosen.
#[test]
fn preset_and_locked_options() {
    let mut run = in_act2(0, TEST_ANCIENT, 20);
    let start = run.state.map.start().coord;
    run.apply(RunAction::ChooseNode(start)).expect("travelable");
    let preset = AncientOption::Relic {
        id: "LAVA_ROCK",
        data: Some(RelicData::LavaRock(lava_rock::Data {
            has_triggered: true,
        })),
    };
    run.phase = Phase::Ancient(AncientRoom {
        kind: AncientKind::TestAncient,
        options: vec![AncientOption::Locked("POOL_3_LOCKED"), preset],
    });
    run.forget_journal();
    assert_eq!(run.legal_actions()[0], RunAction::AncientOption(1));
    assert!(!run.legal_actions().contains(&RunAction::AncientOption(0)));
    assert_eq!(
        run.apply(RunAction::AncientOption(0)),
        Err(RunError::IllegalAction)
    );
    run.apply(RunAction::AncientOption(1)).expect("ported");
    let lava = run.state.relics.last().expect("obtained");
    assert_eq!(lava.kind, RelicKind::LavaRock);
    assert_eq!(
        lava.data,
        RelicData::LavaRock(lava_rock::Data {
            has_triggered: true
        })
    );
}

/// An unported ancient is a stub room, as an unported event is (a made-up id: every real
/// ancient is ported).
#[test]
fn unported_ancient_is_a_stub() {
    let mut run = in_act2(0, "UNPORTED_ANCIENT", 20);
    let start = run.state.map.start().coord;
    run.apply(RunAction::ChooseNode(start)).expect("travelable");
    assert_eq!(
        run.state.room,
        Room::Ancient {
            id: "UNPORTED_ANCIENT"
        }
    );
    assert_eq!(
        run.pending_decision(),
        RunDecision::Unsupported(MapPointType::Ancient)
    );
    assert_eq!(run.state.hp, 20);
    run.apply(RunAction::Proceed).expect("the stub can be left");
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(run.state.map.start().children.iter().collect())
    );
}

/// `RunManager.GenerateRooms` + `ActModel.GenerateRooms` (`RunManager.cs:743-767`,
/// `ActModel.cs:385`): Act 1 has Neow; acts 2 and 3 draw from their own ancients plus a disjoint
/// share of the shared ones (Darv).
#[test]
fn each_act_draws_its_ancient() {
    let mut darv = [0; 3];
    for seed in 0..500 {
        let acts = [&rooms::OVERGROWTH, &rooms::HIVE, &rooms::GLORY];
        let sets = rooms::generate_rooms(&acts, &mut crate::rng::Rng::new(seed), 0);
        let ancients: Vec<&str> = sets.iter().map(|a| a.ancient).collect();
        assert_eq!(ancients[0], "NEOW");
        assert!(["OROBAS", "PAEL", "TEZCATARA", "DARV"].contains(&ancients[1]));
        assert!(["NONUPEIPE", "TANX", "VAKUU", "DARV"].contains(&ancients[2]));
        assert!(ancients[1] != "DARV" || ancients[2] != "DARV");
        for (n, a) in darv.iter_mut().zip(&ancients) {
            *n += usize::from(*a == "DARV");
        }
    }
    assert_eq!(darv[0], 0);
    assert!(
        darv[1] > 0 && darv[2] > 0,
        "Darv reaches both acts: {darv:?}"
    );
}
