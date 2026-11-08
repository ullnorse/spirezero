//! Plays a full Necrobinder vs `FuzzyWurmCrawlerWeak` fight with a trivial policy (play the
//! first legal card, else end the turn) and prints a turn log.
//!
//! `cargo run -p sts2-core --example fight -- [SEED]`

use sts2_core::combat::{Event, Side};
use sts2_core::{Action, Combat, CombatConfig, Status};

#[allow(clippy::print_stdout, reason = "the example's output is its purpose")]
fn main() {
    let seed = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "94LYYA8YDC".to_owned());
    let mut combat = Combat::new(CombatConfig::necrobinder(&seed, 0));
    while matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
        let actions = combat.legal_actions();
        let action = actions
            .iter()
            .find(|a| !matches!(a, Action::EndTurn))
            .unwrap_or(&actions[0])
            .clone();
        combat.apply(action).expect("legal action");
    }
    let state = combat.state();
    for event in combat.events() {
        match event {
            Event::TurnStarted {
                side: Side::Player,
                round,
            } => println!("-- round {round}"),
            Event::TurnStarted {
                side: Side::Enemy, ..
            } => println!("   enemy turn"),
            Event::CardPlayed { card, .. } => {
                println!("   play {:?}", state.card(*card).kind);
            }
            other => println!("   {other:?}"),
        }
    }
    println!(
        "result: {:?}, player hp {}",
        combat.status(),
        state.creature(state.player.creature).hp
    );
}
