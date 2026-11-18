//! RLN40: Trial's "Double Down" is a legal no-op the run mask never offers.

use sts2_core::content::events::EventKind;
use sts2_core::run::{Run, RunAction, RunDecision};
use sts2_rl::RunConfig;
use sts2_rl::run_action::{EVENT_BASE, RunMask, check_run_mask, is_noop_event_option};

fn at_trial() -> Run {
    let mut run = RunConfig::default().run(3).expect("a run");
    while !matches!(run.pending_decision(), RunDecision::Map(_)) {
        let legal = run.legal_actions();
        run.apply(legal[0].clone()).expect("a legal action");
    }
    run.debug_enter_event(EventKind::Trial)
        .expect("Trial begins");
    run
}

fn mask_of(run: &Run) -> RunMask {
    let legal = run.legal_actions();
    let mut mask = RunMask::new();
    mask.fill_run(run, &legal);
    mask.retain_accepted(run, &legal);
    check_run_mask(run, &legal, &mask).expect("the mask is the legal list less the no-op");
    mask
}

#[test]
fn double_down_has_no_bit_and_nothing_loops() {
    let mut run = at_trial();
    // Initial page: Accept (0), Reject (1). Reject opens the page with Double Down at slot 1.
    run.apply(RunAction::EventOption(1)).expect("reject");
    let legal = run.legal_actions();
    assert!(
        legal.contains(&RunAction::EventOption(1)),
        "the engine still lists it"
    );
    assert!(is_noop_event_option(&run, &RunAction::EventOption(1)));
    assert!(!is_noop_event_option(&run, &RunAction::EventOption(0)));
    let mask = mask_of(&run);
    assert!(mask.contains(EVENT_BASE), "Accept stays");
    assert!(!mask.contains(EVENT_BASE + 1), "Double Down is masked");
    assert_eq!(mask.count(), 1);
    // The only move leaves the page.
    run.apply(RunAction::EventOption(0)).expect("accept");
    assert!(
        !is_noop_event_option(&run, &RunAction::EventOption(1)),
        "verdict pages keep both"
    );
    assert_eq!(mask_of(&run).count(), 2);
}

#[test]
fn first_page_reject_stays_legal() {
    let run = at_trial();
    let mask = mask_of(&run);
    assert!(mask.contains(EVENT_BASE) && mask.contains(EVENT_BASE + 1));
}
