//! Test-only power: `SwipePower` (`Models.Powers/SwipePower.cs`) without its monster, to exercise
//! run-deck theft until `ThievingHopper` is ported. `steal` is `Steal` (`SwipePower.cs:69-78`);
//! `before_death` is `BeforeDeath` (`SwipePower.cs:50-67`): the stolen card comes back as a
//! `SpecialCardReward`. An escape drops the power without a callback, so the card stays gone.

#![cfg(test)]

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, ExtraReward, Flow};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// Test-only: holds a card stolen from the run deck.
pub struct TestSwipe;

/// The instance's `StolenCard`: the combat card, whose `deck_version` is the deck card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub stolen: Option<CardId>,
}

/// `ThievingHopper.ThieveryMove`'s theft of one card (`ThievingHopper.cs:223-244`):
/// `RemoveFromCombat`, then `swipe.Steal(card)` and the power applied to `owner`.
pub fn steal(eng: &mut Engine<'_>, owner: CreatureId, card: CardId) -> Flow {
    eng.remove_from_combat(&[card])?;
    if let Some(deck_index) = eng.s.card(card).deck_version() {
        eng.remove_from_deck(deck_index);
    }
    let instance =
        eng.apply_power_instance(owner, PowerKind::TestSwipe, Decimal::ONE, Some(owner))?;
    if let Some(me) = instance {
        eng.s
            .set_power_data(me.id, PowerData::TestSwipe(Data { stolen: Some(card) }));
    }
    Ok(())
}

impl PowerModel for TestSwipe {
    fn name(&self) -> &'static str {
        "Test Swipe"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::TestSwipe(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for TestSwipe {
    type Me = PowerRef;

    fn before_death(&self, eng: &mut Engine<'_>, me: PowerRef, creature: CreatureId) -> Flow {
        if me.owner != creature {
            return Ok(());
        }
        let PowerData::TestSwipe(data) = *eng.s.power_data(me.id) else {
            unreachable!("TestSwipe holds its own data");
        };
        if let Some(deck_index) = data.stolen.and_then(|c| eng.s.card(c).deck_version()) {
            eng.s
                .extra_rewards
                .push(ExtraReward::SpecialCard(deck_index));
        }
        Ok(())
    }
}
