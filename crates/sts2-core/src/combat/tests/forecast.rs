//! The read-only forecast getters (`Hooked::forecast_into`, `Hooked::intents_of_move_into`)
//! that the RL observation's forecast reads (RLN16).

use super::*;
use crate::combat::{ForecastStep, IntentScratch};
use crate::rng::RunRngSet;

#[test]
fn the_current_move_by_index_shows_the_intents_it_shows_now() {
    let c = new_combat();
    let e = enemy(&c);
    let m = c.state.creature(e).monster.as_ref().expect("a monster");
    assert!(m.rolled && m.machine.forced.is_none());
    let (mut intents, mut bases) = (Vec::new(), Vec::new());
    c.state.hooked().intents_of_move_into(
        e,
        m.machine.current,
        &mut IntentScratch::default(),
        &mut intents,
        &mut bases,
    );
    assert_eq!(intents, c.state.intents(e));
}

#[test]
fn a_forecast_changes_nothing_and_reads_no_random_number() {
    let c = new_combat();
    let e = enemy(&c);
    let forecast = |s: &CombatState| {
        let mut steps: Vec<ForecastStep> = Vec::new();
        s.hooked().forecast_into(
            e,
            3,
            &mut IntentScratch::default(),
            &mut steps,
            &mut Vec::new(),
        );
        steps
    };
    let before = c.state.clone();
    let steps = forecast(&c.state);
    assert_eq!(c.state, before, "read-only");
    assert_eq!(steps.len(), 3);
    assert!(steps.iter().all(|s| (0.0..=1.0).contains(&s.certainty)));
    assert!(
        steps.windows(2).all(|w| w[1].certainty <= w[0].certainty),
        "certainty is a product of shares"
    );
    let mut other = before;
    other.rng = RunRngSet::new("FORECAST");
    for creature in &mut other.creatures {
        if let Some(m) = &mut creature.monster {
            m.rng = Rng::new(0x5EED);
        }
    }
    assert_eq!(forecast(&other), steps, "no random number is read");
}
