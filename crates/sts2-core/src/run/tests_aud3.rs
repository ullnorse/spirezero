//! Batch AUD3: the Hatch rest option (`ByrdonisEgg`) and the `Byrdpip` relic and pet, against the
//! decomp (`Entities.RestSite/HatchRestSiteOption.cs`, `Models.Relics/Byrdpip.cs`,
//! `Models.Monsters/Byrdpip.cs`).

use super::*;
use crate::content::monsters::MonsterKind;

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

fn at_rest_site(run: &mut Run) {
    run.state.room = Room::RestSite;
    let options = run.rest_options();
    run.phase = Phase::RestSite(RestSite {
        options,
        used: false,
        pending: None,
    });
}

fn rest_ids(run: &Run) -> Vec<&'static str> {
    match run.pending_decision() {
        RunDecision::RestSite(options) => options.iter().map(|o| o.kind.id()).collect(),
        d => panic!("not at a rest site: {d:?}"),
    }
}

fn deck_ids(run: &Run) -> Vec<&'static str> {
    run.state.deck.iter().map(|c| c.id).collect()
}

#[test]
fn each_egg_adds_a_hatch_option_and_hatching_turns_every_egg_into_a_swoop() {
    let mut run = after_neow();
    let before = deck_ids(&run).len();
    run.state.add_card(RunCard::new("BYRDONIS_EGG"));
    run.state.add_card(RunCard::new("BYRDONIS_EGG"));
    at_rest_site(&mut run);
    assert_eq!(rest_ids(&run), ["HEAL", "SMITH", "HATCH", "HATCH"]);
    run.apply(RunAction::RestOption(2)).expect("hatch");
    // `HatchRestSiteOption.OnSelect`: `RelicCmd.Obtain<Byrdpip>`, whose `AfterObtained` runs
    // `CardCmd.TransformTo<ByrdSwoop>` on each egg; the replacements join the end of the deck.
    assert_eq!(run.state.relics.last().map(|r| r.id()), Some("BYRDPIP"));
    let deck = deck_ids(&run);
    assert_eq!(deck.len(), before + 2);
    assert_eq!(&deck[before..], ["BYRD_SWOOP", "BYRD_SWOOP"]);
    assert!(!deck.contains(&"BYRDONIS_EGG"));
    assert!(matches!(run.pending_decision(), RunDecision::Map(_)));
}

#[test]
fn byrdpip_is_a_pet_with_9999_hp_in_every_fight() {
    let mut run = after_neow();
    run.state.relics.push(RunRelic::new(RelicKind::Byrdpip));
    let first = run
        .state
        .map
        .start()
        .children
        .iter()
        .min()
        .expect("a first room");
    run.apply(RunAction::ChooseNode(first)).expect("a fight");
    let cs = run.combat().expect("in combat").state();
    let pet = cs
        .player
        .pets
        .iter()
        .map(|&id| cs.creature(id))
        .find(|c| {
            matches!(
                c.monster.as_ref().map(|m| m.kind),
                Some(MonsterKind::Byrdpip(_))
            )
        })
        .expect("Byrdpip is a pet");
    assert_eq!((pet.hp, pet.max_hp), (9999, 9999));
    assert_eq!(pet.pet_owner, Some(cs.player.creature));
    assert!(cs.allies.contains(&pet.id));
    // Osty is summoned by `BoundPhylactery` first, so the pet follows it.
    assert_eq!(cs.allies.len(), 3);
}
