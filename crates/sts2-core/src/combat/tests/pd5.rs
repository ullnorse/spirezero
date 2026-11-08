//! Defect energy, draw and kill-check cards (PD5; paths under `ref/decomp/MegaCrit.Sts2.Core.`,
//! spec `docs/specs/defect.md` batch 5): paths the referee does not reach.

use super::*;
use crate::content::cards::{CardData, genetic_algorithm};

fn defect_combat() -> Combat {
    Combat::new(
        CombatConfig::new(Character::Defect, SEED, 0)
            .expect("Defect's starter is ported")
            .with_events(),
    )
}

#[test]
fn rocket_punch_discounts_itself_when_the_owner_generates_a_status() {
    // RocketPunch.cs:AfterCardGeneratedForCombat: `creator == Owner`, a Status: the listener's own
    // `EnergyCost.AddUntilPlayed(-1)` (the referee shows RocketPunch dropping to 1, the Status untouched);
    // a monster's Status (no creator) changes nothing.
    let mut c = defect_combat();
    let p = player(&c);
    let punch = give_card(&mut c, CardKind::RocketPunch, 0);
    let slimed = with_engine(&mut c, |eng| {
        let slimed = eng.create_card(CardKind::Slimed);
        eng.add_generated_card_to_combat_at(slimed, PileType::Discard, None, Position::Bottom)?;
        Ok(slimed)
    });
    assert_eq!(c.state.energy_cost(slimed), 1);
    assert_eq!(c.state.energy_cost(punch), 2);
    let own = with_engine(&mut c, |eng| {
        let slimed = eng.create_card(CardKind::Slimed);
        eng.add_generated_card_to_combat_at(slimed, PileType::Discard, Some(p), Position::Bottom)?;
        Ok(slimed)
    });
    assert_eq!(c.state.energy_cost(own), 1);
    assert_eq!(c.state.energy_cost(punch), 1);
}

#[test]
fn genetic_algorithm_starts_from_its_deck_version_and_buffs_it() {
    // GeneticAlgorithm.cs: the block is `1 + IncreasedBlock` (a saved property); OnPlay buffs the
    // card and its deck version by `Increase` (3).
    let mut config = CombatConfig::new(Character::Defect, SEED, 0).expect("Defect starts");
    config.deck[0] = DeckCard {
        counter: 4,
        ..DeckCard::new(CardKind::GeneticAlgorithm)
    };
    let mut c = Combat::new(config);
    let ga = CardId(0);
    assert_eq!(c.state.card(ga).deck_version(), Some(0));
    assert_eq!(
        c.state.card(ga).data,
        CardData::GeneticAlgorithm(genetic_algorithm::Data { increased_block: 4 })
    );
    let p = player(&c);
    c.state.creature_mut(p).block = 0;
    with_engine(&mut c, |eng| {
        eng.add_to_pile(ga, PileType::Hand, Position::Bottom)
    });
    c.apply(Action::PlayCard {
        card: ga,
        target: None,
    })
    .expect("legal");
    assert_eq!(c.state.creature(p).block, 5);
    assert_eq!(c.state.deck[0].counter, 7);
}
