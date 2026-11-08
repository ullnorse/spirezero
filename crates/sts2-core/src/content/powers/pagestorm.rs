//! `PagestormPower` (`Models.Powers/PagestormPower.cs`): drawing an Ethereal card draws `Amount`
//! more cards.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, Engine, Flow};
use crate::content::cards::CardKeyword;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Pagestorm;

impl PowerModel for Pagestorm {
    fn name(&self) -> &'static str {
        "Pagestorm"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Pagestorm {
    type Me = PowerRef;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner
            || !eng.hooked().has_keyword(card, CardKeyword::Ethereal)
        {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.draw(amount, false)?;
        Ok(())
    }
}
