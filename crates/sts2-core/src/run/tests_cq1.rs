//! CQ1: the numbers a rest-site option's description shows ([`RunState::rest_option_vars`]).

use super::*;

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

fn obtain(run: &mut Run, id: &'static str) {
    if let Some(phase) = run.obtain_relic(id).expect("ported") {
        run.phase = phase;
    }
}

#[test]
fn rest_option_vars_follow_the_relics() {
    let mut run = after_neow();
    let heal = |run: &Run| run.state.rest_option_vars(RestOptionKind::Heal);
    let base = Decimal::from(run.state.max_hp) * Decimal::new(3, 1);
    assert_eq!(heal(&run), [("Heal", base)]);
    // RegalPillow: ModifyRestSiteHealAmount adds 15.
    obtain(&mut run, "REGAL_PILLOW");
    assert_eq!(heal(&run), [("Heal", base + Decimal::from(15))]);

    obtain(&mut run, "GIRYA");
    let lifts = |run: &Run| run.state.rest_option_vars(RestOptionKind::Lift);
    assert_eq!(lifts(&run), [("LiftsLeft", Decimal::from(3))]);
    let girya = run
        .state
        .relics
        .iter()
        .position(|r| r.kind == RelicKind::Girya)
        .expect("obtained");
    girya::lift(&mut run.state, girya);
    assert_eq!(lifts(&run), [("LiftsLeft", Decimal::from(2))]);

    assert_eq!(
        run.state.rest_option_vars(RestOptionKind::Cook),
        [("Cards", Decimal::from(2)), ("MaxHp", Decimal::from(5))]
    );
    assert_eq!(
        run.state.rest_option_vars(RestOptionKind::Smith),
        [("Count", Decimal::ONE)]
    );
}

#[test]
fn a_victory_that_meets_unported_content_leaves_the_run_unchanged() {
    // The winning combat action is applied in place; the victory's relic hook then fails
    // after changing the run, which returns the error instead of panicking and restores the run.
    let mut run = after_neow();
    run.state
        .relics
        .push(RunRelic::new(RelicKind::TestUnportedTrophy));
    run.forget_journal();
    let node = run
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, RunAction::ChooseNode(_)))
        .expect("a first node");
    run.apply(node).expect("the first room is a fight");
    let Phase::Combat(combat) = &mut run.phase else {
        panic!("a fight");
    };
    combat.with_engine(|eng| {
        for e in eng.s.enemies.clone() {
            eng.s.creature_mut(e).hp = 1;
        }
        Ok(())
    });
    let winning = combat
        .legal_actions()
        .into_iter()
        .find(|a| {
            let mut c = (**combat).clone();
            c.apply(a.clone()) == Ok(Status::Won)
        })
        .expect("a play that wins");
    run.forget_journal();
    let before = run.clone();
    assert_eq!(
        run.apply(RunAction::Combat(winning)),
        Err(RunError::UnsupportedRelic("NOT_A_PORTED_RELIC"))
    );
    assert_eq!(run, before);
}
