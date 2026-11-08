//! `SwipePower` (`Models.Powers/SwipePower.cs`): holds a card the `ThievingHopper` stole from the
//! run deck (`Instanced` buff). `steal` is `Steal` (`SwipePower.cs:69-78`); `before_death` is
//! `BeforeDeath` (`SwipePower.cs:50-67`): the stolen card comes back as a `SpecialCardReward`.
//! An escape drops the power without a callback, so the card stays gone. The loot history
//! (`MarkLootStolen`/`MarkLootReturned`) is not modelled.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, ExtraReward, Flow};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// Holds a card stolen from the run deck.
pub struct Swipe;

/// The instance's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `StolenCard`: the combat card, whose `deck_version` is the deck card.
    pub stolen: Option<CardId>,
    /// `Target`: the card owner's creature.
    pub target: Option<CreatureId>,
}

/// `ThievingHopper.ThieveryMove`'s second half for one stolen card (`ThievingHopper.cs`): a
/// new `SwipePower`, `swipe.Steal(card)` (sets `Target` and `StolenCard`, then
/// `RemoveFromDeck(card.DeckVersion)`), then the power applied to `owner`. The card already
/// left combat (`RemoveFromCombat`).
pub fn steal(eng: &mut Engine<'_>, owner: CreatureId, card: CardId) -> Flow {
    let target = eng.s.player.creature;
    if let Some(deck_index) = eng.s.card(card).deck_version() {
        eng.remove_from_deck(deck_index);
    }
    let instance = eng.apply_power_instance(owner, PowerKind::Swipe, Decimal::ONE, Some(owner))?;
    if let Some(me) = instance {
        eng.s.set_power_data(
            me.id,
            PowerData::Swipe(Data {
                stolen: Some(card),
                target: Some(target),
            }),
        );
    }
    Ok(())
}

impl PowerModel for Swipe {
    fn name(&self) -> &'static str {
        "Swipe"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Swipe(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Swipe {
    type Me = PowerRef;

    fn before_death(&self, eng: &mut Engine<'_>, me: PowerRef, creature: CreatureId) -> Flow {
        if me.owner != creature {
            return Ok(());
        }
        let PowerData::Swipe(data) = *eng.s.power_data(me.id) else {
            unreachable!("Swipe holds its own data");
        };
        if let Some(deck_index) = data.stolen.and_then(|c| eng.s.card(c).deck_version()) {
            eng.s
                .extra_rewards
                .push(ExtraReward::SpecialCard(deck_index));
        }
        Ok(())
    }
}
