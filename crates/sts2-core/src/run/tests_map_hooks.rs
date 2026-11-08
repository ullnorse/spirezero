//! Map hooks (C7): `ModifyGeneratedMap(+Late)`, `ModifyUnknownMapPointRoomTypes`,
//! `GoldenPathActMap` and `RunManager.GenerateMap` after an obtain, through the test-only relics
//! `TestGoldenPath`, `TestMapMarker` and `TestUnknownShops`.

use super::map::{MapCoord, MapPointType};
use super::*;
use crate::content::relics::{RelicData, RelicKind, test_map_marker};

/// `T6S0` on the Act 1 map after Neow, owning `kinds` (no `AfterObtained` run).
fn with_relics(kinds: &[RelicKind]) -> Run {
    let mut run = Run::new(
        Character::Necrobinder,
        "T6S0",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("T6S0 starts");
    run.apply(RunAction::AncientOption(0))
        .expect("Phial Holster is ported");
    run.state
        .relics
        .extend(kinds.iter().map(|&k| RunRelic::new(k)));
    run
}

const fn at(col: usize, row: usize) -> MapCoord {
    MapCoord { col, row }
}

fn marked(run: &Run) -> Option<(usize, MapCoord)> {
    let data = run
        .state
        .relics
        .iter()
        .find(|r| r.kind == RelicKind::TestMapMarker)
        .expect("a TestMapMarker")
        .data;
    let RelicData::TestMapMarker(test_map_marker::Data { marked }) = data else {
        unreachable!("TestMapMarker data");
    };
    marked
}

#[test]
fn golden_path_is_one_column_of_sixteen_fixed_rooms() {
    use MapPointType::{Ancient, Boss, Elite, Monster, RestSite, Shop, Treasure, Unknown};
    let map = ActMap::golden_path();
    let kinds: Vec<MapPointType> = map.points().iter().map(|p| p.kind).collect();
    assert_eq!(
        kinds,
        [
            Monster, Unknown, Monster, RestSite, Monster, RestSite, Unknown, Treasure, Unknown,
            Treasure, Unknown, Shop, Elite, RestSite, Elite, RestSite,
        ]
    );
    assert_eq!((map.start().coord, map.start().kind), (at(3, 0), Ancient));
    assert_eq!(map.start().children, [at(3, 1)]);
    assert_eq!((map.boss().coord, map.boss().kind), (at(3, 17), Boss));
    assert_eq!(map.boss().parents, [at(3, 16)]);
    assert!(map.second_boss().is_none());
    for (i, p) in map.points().iter().enumerate() {
        assert_eq!(p.coord, at(3, i + 1));
        assert_eq!(p.parents, [at(3, i)]);
        assert_eq!(p.children, [at(3, i + 2)]);
    }
    assert!(map.has_point(at(3, 17)) && !map.has_point(at(2, 5)));
}

#[test]
fn an_obtain_regenerates_the_current_map_and_drops_stale_coords() {
    let mut run = with_relics(&[]);
    let standard = run.state.map.clone();
    assert_ne!(standard, ActMap::golden_path());
    let stale = standard.points()[0].coord;
    assert_ne!(stale.col, 3);
    run.state.visited.push(stale);
    run.state
        .relics
        .push(RunRelic::new(RelicKind::TestGoldenPath));
    let me = run.state.relics.len() - 1;
    RelicKind::TestGoldenPath
        .model()
        .after_obtained(&mut RunCtx::new(&mut run), me)
        .expect("regenerates");
    assert_eq!(run.state.map, ActMap::golden_path());
    assert_eq!(run.state.visited, [at(3, 0)]);
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(smallvec::smallvec![at(3, 1)])
    );
    // `GenerateMap` without a listener draws the same map again from `act_1_map`.
    run.state.relics.pop();
    run.state.generate_map().expect("generates");
    assert_eq!(run.state.map, standard);
}

#[test]
fn late_listeners_see_the_map_every_early_listener_made() {
    // The marker comes first in the relic list but its late pass runs after every early one.
    let mut run = with_relics(&[RelicKind::TestMapMarker, RelicKind::TestGoldenPath]);
    run.state.generate_map().expect("generates");
    assert_eq!(marked(&run), Some((0, at(3, 8))));
    // Entering Act 2 generates through both passes too.
    run.enter_next_act().expect("Act 2");
    assert_eq!(run.state.map, ActMap::golden_path());
    assert_eq!(marked(&run), Some((1, at(3, 8))));
}

#[test]
fn spoils_map_and_relics_replace_act_2_in_listener_order() {
    let mut run = with_relics(&[RelicKind::TestMapMarker]);
    run.state.deck.push(RunCard::new("SPOILS_MAP"));
    run.enter_next_act().expect("Act 2");
    let spoils = ActMap::generate_spoils(&run.state.rng, 1, 0).expect("spoils map");
    assert_eq!(run.state.map, spoils);
    let treasure = spoils
        .points()
        .iter()
        .find(|p| p.kind == MapPointType::Treasure)
        .map(|p| p.coord);
    assert_eq!(marked(&run).map(|(_, c)| c), treasure);
    // The deck's `SpoilsMap` acts before the relics: a relic's map wins.
    let mut run = with_relics(&[RelicKind::TestGoldenPath]);
    run.state.deck.push(RunCard::new("SPOILS_MAP"));
    run.enter_next_act().expect("Act 2");
    assert_eq!(run.state.map, ActMap::golden_path());
}

#[test]
fn unknown_room_types_go_through_the_relics() {
    let mut s = with_relics(&[RelicKind::TestUnknownShops]).state;
    let mut types = UnknownMapPointOdds::room_types();
    hooks::modify_unknown_map_point_room_types(&s, &mut types);
    assert_eq!(types, [RoomType::Shop]);
    let roll = s
        .unknown_odds
        .roll(&types, &mut s.rng[RunRngType::UnknownMapPoint]);
    assert_eq!(roll, RoomType::Shop);
}
