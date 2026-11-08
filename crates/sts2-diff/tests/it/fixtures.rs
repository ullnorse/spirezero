//! Replays every fixture against the engine. A fixture without a recorded divergence must
//! match the referee's trace; fixtures with one get an `#[ignore]`d test naming it, so a fix
//! just un-ignores the test.

use sts2_diff::fixture::{Fixture, RunFixture, dir, run_dir};

fn load(name: &str) -> Fixture {
    Fixture::load(&dir().join(format!("{name}.json"))).expect("fixture loads")
}

/// Asserts the engine reproduces the fixture's referee trace. A fixture with a recorded
/// divergence gets a test that calls this and is `#[ignore = "<field> diverges: <cause>"]`.
fn assert_matches(name: &str, fixture: &Fixture) {
    if let Some(divergence) = fixture.check().expect("engine replays the actions") {
        panic!("{name}: {divergence}");
    }
}

#[test]
fn matching_fixtures_replay() {
    let mut checked = 0;
    for entry in std::fs::read_dir(dir()).expect("fixtures dir") {
        let path = entry.expect("dir entry").path();
        let fixture = Fixture::load(&path).expect("fixture loads");
        if fixture.divergence.is_some() {
            continue;
        }
        assert_matches(&path.display().to_string(), &fixture);
        checked += 1;
    }
    assert!(checked > 0, "no matching fixtures found");
}

#[test]
fn fixture_with_a_recorded_divergence_is_detected() {
    // The comparison must not be vacuous: corrupting the referee trace has to be caught.
    let mut fixture = load("match_won");
    fixture.referee_trace[1]["state"]["player"]["energy"] = 99.into();
    let divergence = fixture.check().expect("engine replays the actions");
    assert_eq!(divergence.expect("divergence").path, "state.player.energy");
}

#[test]
fn card_instance_state_and_rng_counters_are_compared() {
    let fixture = load("match_won");
    let mut cost = fixture.clone();
    cost.referee_trace[0]["state"]["hand"][0]["cost"] = 99.into();
    assert_eq!(
        cost.check().expect("replays").expect("divergence").path,
        "state.hand[0].cost"
    );
    let mut keywords = fixture.clone();
    keywords.referee_trace[0]["state"]["draw"][0]["keywords"] = serde_json::json!(["Exhaust"]);
    assert_eq!(
        keywords.check().expect("replays").expect("divergence").path,
        "state.draw[0].keywords.len"
    );
    let mut rng = fixture;
    let last = rng.referee_trace.len() - 1;
    rng.referee_trace[last]["state"]["rng"]["run"]["Shuffle"] = 999.into();
    assert_eq!(
        rng.check().expect("replays").expect("divergence").path,
        "state.rng.run.Shuffle"
    );
}

#[test]
fn the_torn_down_terminal_state_is_compared_in_full() {
    // After a win the referee reports emptied piles and removed enemies; anything else is caught.
    let mut fixture = load("match_won");
    let last = fixture.referee_trace.len() - 1;
    assert_eq!(fixture.referee_trace[last]["state"]["combat_over"], true);
    fixture.referee_trace[last]["state"]["player"]["block"] = 5.into();
    assert_eq!(
        fixture.check().expect("replays").expect("divergence").path,
        "state.player.block"
    );
}

#[test]
fn a10_ascenders_bane() {
    // Fixed in C11: the combat deck has AscendersBane from ascension 5.
    assert_matches("a10_ascenders_bane", &load("a10_ascenders_bane"));
}

#[test]
fn a10_start_hp() {
    // Fixed in RL5: WearyTraveler's Neow heal (52 of 66 HP from ascension 2).
    assert_matches("a10_start_hp", &load("a10_start_hp"));
}

#[test]
fn won_fogmog_fight_keeps_the_dead_eye_in_the_enemy_list() {
    // `CombatManager.EndCombatInternal` does not clear `CombatState.Enemies`, so the Eye With
    // Teeth (dead, waiting to revive) is still listed after the Fogmog's death wins the fight.
    let name = "fogmog_won_eye_stays";
    let fixture = load(name);
    let last = fixture.referee_trace.last().expect("trace has steps");
    assert_eq!(last["state"]["combat_over"], true);
    assert_eq!(last["state"]["enemies"].as_array().map(Vec::len), Some(1));
    assert_matches(name, &fixture);
}

#[test]
fn run_fixtures_replay() {
    let mut checked = 0;
    for entry in std::fs::read_dir(run_dir()).expect("run fixtures dir") {
        let path = entry.expect("dir entry").path();
        // `candidates/` holds unpinned sign-off divergences (SG1), which fail by design.
        if path.is_dir() {
            continue;
        }
        let fixture = RunFixture::load(&path).expect("run fixture loads");
        if let Some(divergence) = fixture.check().expect("engine replays the run") {
            panic!("{}: {divergence}", path.display());
        }
        checked += 1;
    }
    assert!(checked > 0, "no run fixtures found");
}

/// A `VoidForm` whose own end of turn wins the fight breaks the referee's action queue for the
/// next fight (PX15, `h4-juzu_bracelet` Defect A10 `RL2S6`, Regent `RL2S157`): the harness
/// refuses that play, here with 3 energy, two Lightning orbs and a Chomper at 3 HP.
#[test]
fn a_void_form_that_wins_on_its_own_end_of_turn_is_refused() {
    use sts2_core::content::cards::CardKind;
    use sts2_core::{Action, Character, run::RunAction};
    use sts2_diff::run_session::{act1_of, replay_session};
    let fixture =
        RunFixture::load(&run_dir().join("void_form_win_before_play.json")).expect("loads");
    let actions = fixture.actions["actions"].as_array().expect("actions");
    let (session, _) = replay_session(
        Character::Defect,
        &fixture.seed,
        10,
        (false, act1_of(&fixture.actions).expect("act1")),
        actions,
    )
    .expect("replays");
    let combat = session.run().combat().expect("in the fight");
    let state = combat.state();
    let void_form = state
        .player
        .hand
        .iter()
        .copied()
        .find(|&c| state.card(c).kind == CardKind::VoidForm)
        .expect("Void Form in hand");
    let play = RunAction::Combat(Action::PlayCard {
        card: void_form,
        target: None,
    });
    assert_eq!(
        session.check(&play),
        Err("referee card VOID_FORM".to_owned())
    );
}

/// A Void Form whose end of turn opens Entropy's transform at the next turn's start breaks the
/// referee (sts2-cli's hand dump throws `Collection was modified`, PX20, Necrobinder A10
/// `ROT261023S3`): the harness refuses that play too.
#[test]
fn a_void_form_that_opens_a_next_turn_choice_is_refused() {
    use sts2_core::content::cards::CardKind;
    use sts2_core::{Action, Character, run::RunAction};
    use sts2_diff::run_session::{act1_of, replay_session};
    let fixture =
        RunFixture::load(&run_dir().join("void_form_entropy_next_turn.json")).expect("loads");
    let actions = fixture.actions["actions"].as_array().expect("actions");
    let (session, _) = replay_session(
        Character::Necrobinder,
        &fixture.seed,
        10,
        (false, act1_of(&fixture.actions).expect("act1")),
        actions,
    )
    .expect("replays");
    let combat = session.run().combat().expect("in the fight");
    let state = combat.state();
    let void_form = state
        .player
        .hand
        .iter()
        .copied()
        .find(|&c| state.card(c).kind == CardKind::VoidForm)
        .expect("Void Form in hand");
    let play = RunAction::Combat(Action::PlayCard {
        card: void_form,
        target: None,
    });
    assert_eq!(
        session.check(&play),
        Err("referee card VOID_FORM".to_owned())
    );
}
