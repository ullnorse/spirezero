//! C9: the act flow after a boss (`NRewardsScreen.OnProceedButtonPressed`,
//! `ActChangeSynchronizer.MoveToNextAct`, `RunManager.EnterNextAct`): the next act, the A10
//! second boss, and the victory room.

use super::*;

/// `T6S0` at `ascension` after Neow (Phial Holster), moved on to act index `act` as
/// `EnterNextAct` leaves it.
fn in_act(ascension: u8, act: usize) -> Run {
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
    for _ in 0..act {
        run.enter_next_act().expect("the act's map generates");
    }
    run
}

/// The run on `coord` (a boss point) with the boss's rewards done, then `Proceed`.
fn leave_boss_rewards(run: &mut Run, coord: MapCoord) {
    run.state.visited.push(coord);
    run.state.act_floor = coord.row + 1;
    run.state.room = Room::Combat {
        room_type: RoomType::Boss,
        encounter: run.state.act().boss.id,
    };
    run.phase = Phase::Rewards(Vec::new());
    run.apply(RunAction::Proceed)
        .expect("the rewards can be left");
}

/// Act 2's boss moves the run on to Glory's map screen.
#[test]
fn act2_boss_enters_act3() {
    let mut run = in_act(0, 1);
    let boss = run.state.map.boss().coord;
    leave_boss_rewards(&mut run, boss);
    let s = run.state();
    assert_eq!((s.act_index, s.act_floor), (2, 0));
    assert_eq!(s.act().act.id, "GLORY");
    assert_eq!(s.map, ActMap::generate(&s.rng, 2, 0).expect("Glory's map"));
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(smallvec::smallvec![s.map.start().coord])
    );
}

/// Below `DoubleBoss` the last act's boss wins the run: `ActFloor++`, then `TheArchitect`'s event
/// room (counted as an event room visit, one `Niche` draw), where the run ends.
#[test]
fn last_boss_wins_the_run() {
    let mut run = in_act(0, 2);
    assert!(run.state.map.second_boss().is_none());
    let boss = run.state.map.boss().coord;
    let floor = run.state.total_floor();
    let niche = run.state.rng[RunRngType::Niche].counter();
    leave_boss_rewards(&mut run, boss);
    assert_eq!(run.pending_decision(), RunDecision::Victory);
    // The combat layout's `Architect` rolls its max HP (PX16).
    assert_eq!(run.state.rng[RunRngType::Niche].counter(), niche + 1);
    assert_eq!(run.legal_actions(), Vec::<RunAction>::new());
    let s = run.state();
    assert_eq!((s.act_index, s.act_floor), (2, boss.row + 2));
    assert_eq!(s.total_floor(), floor);
    assert_eq!(s.room, Room::Event { id: THE_ARCHITECT });
    assert_eq!(s.act().events_visited, 1);
}

/// From `DoubleBoss` the first of the last act's two bosses returns to the map, whose only next
/// point is the second boss; that one wins the run.
#[test]
fn a10_first_boss_proceeds_to_the_second() {
    let mut run = in_act(10, 2);
    let boss = run.state.map.boss().coord;
    let second = run
        .state
        .map
        .second_boss()
        .expect("A10 has a second boss")
        .coord;
    leave_boss_rewards(&mut run, boss);
    assert_eq!(
        run.pending_decision(),
        RunDecision::Map(smallvec::smallvec![second])
    );
    assert_eq!((run.state.act_index, &run.state.room), (2, &Room::Map));
    leave_boss_rewards(&mut run, second);
    assert_eq!(run.pending_decision(), RunDecision::Victory);
}

/// A `SpoilsMap` in the deck as Act 2's map is generated notes its treasure point (the quest
/// marker); that treasure room pays its 600 gold after the chest's and removes the card.
#[test]
fn spoils_map_quest_completes_in_its_treasure_room() {
    let mut run = in_act(0, 0);
    run.state.add_card(RunCard::new(SPOILS_MAP));
    run.enter_next_act().expect("the spoils map generates");
    let s = &run.state;
    let treasure = s
        .map
        .points()
        .iter()
        .find(|p| p.kind == MapPointType::Treasure)
        .expect("a treasure point")
        .coord;
    assert_eq!(s.spoils_quest, Some(treasure));
    let card = *s.deck.last().expect("the card");
    assert_eq!((card.id, card.counter), (SPOILS_MAP, 1));
    let (gold, deck) = (s.gold, s.deck.len());
    let mut chest = s.player_rng.clone();
    let chest_gold = chest[PlayerRngType::Rewards].next_int_range(42, 53);
    run.state.visited.push(treasure);
    run.enter_treasure();
    let s = &run.state;
    assert_eq!(s.gold, gold + chest_gold + 600);
    assert_eq!(s.deck.len(), deck - 1);
    assert_eq!(s.spoils_quest, None);
}
