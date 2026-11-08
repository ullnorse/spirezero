//! `NightmarePower` (`Models.Powers/NightmarePower.cs`): an `Instanced` buff (`Counter`). It holds
//! a clone (affliction cleared) of the card `Nightmare` selected; before the owner's next hand
//! draw it adds `Amount` clones of it to the hand, then removes itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, Engine, Flow, PileType};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};

pub struct Nightmare;

/// `NightmarePower.Data.selectedCard`: `None` between the power's application and
/// `Nightmare.OnPlay`'s `SetSelectedCard`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub selected_card: Option<CardId>,
}

impl Nightmare {
    /// `SetSelectedCard`: stores a clone with its affliction cleared in the instance's data.
    pub(crate) fn set_selected_card(eng: &mut Engine<'_>, me: PowerRef, card: CardId) {
        let clone = eng.create_clone(card);
        eng.clear_affliction(clone);
        eng.s.set_power_data(
            me.id,
            PowerData::Nightmare(Data {
                selected_card: Some(clone),
            }),
        );
    }
}

impl PowerModel for Nightmare {
    fn name(&self) -> &'static str {
        "Nightmare"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Nightmare(Data {
            selected_card: None,
        })
    }
}

#[sts2_macros::hooks]
impl Hooks for Nightmare {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player is the owner.
        let PowerData::Nightmare(Data {
            selected_card: Some(card),
        }) = *eng.s.power_data(me.id)
        else {
            unreachable!("a Nightmare instance holds its selected card");
        };
        let amount = eng.s.power(me.owner, me.id).amount;
        for _ in 0..amount {
            let clone = eng.create_clone(card);
            eng.add_generated_card_to_combat(clone, PileType::Hand, Some(eng.s.player.creature))?;
        }
        eng.remove_power(me.owner, me.id)
    }
}
