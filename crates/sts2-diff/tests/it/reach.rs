//! The RLB3 positive control: one replayable ascension-0 victory.

use sts2_diff::reach::{End, Fixture, replay};

/// The recorded run wins on the engine: seed `R35W61N88ZUQ`, Ironclad, A0, Overgrowth.
#[test]
fn the_recorded_a0_run_reaches_victory() {
    let text = include_str!("../fixtures/rlb3-victory.txt");
    let fixture = Fixture::parse(text).expect("the fixture parses");
    assert_eq!(fixture.seed, "R35W61N88ZUQ");
    assert_eq!(fixture.character.name(), "Ironclad");
    assert_eq!(fixture.ascension, 0);
    assert_eq!(fixture.actions.len(), 399);

    let outcome = replay(
        fixture.character,
        &fixture.seed,
        fixture.ascension,
        fixture.act1,
        &fixture.actions,
    )
    .expect("the replay applies every action");
    assert_eq!(outcome.end, End::Victory);
    assert_eq!(
        (outcome.act, outcome.floor, outcome.hp, outcome.max_hp),
        (3, 48, 18, 98)
    );
}
