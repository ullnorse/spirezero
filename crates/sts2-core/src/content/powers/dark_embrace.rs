//! `DarkEmbracePower` (`Models.Powers/DarkEmbracePower.cs`): a buff (`Counter`); the owner draws
//! `Amount` cards whenever one of their cards is exhausted. Ethereal exhaustion at turn end is
//! counted instead, and drawn after the side's turn ends.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct DarkEmbrace;

/// `DarkEmbracePower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `etherealCount`.
    pub ethereal_count: i32,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::DarkEmbrace(data) => data,
        _ => unreachable!("a DarkEmbrace instance holds DarkEmbrace data"),
    }
}

impl PowerModel for DarkEmbrace {
    fn name(&self) -> &'static str {
        "Dark Embrace"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::DarkEmbrace(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for DarkEmbrace {
    type Me = PowerRef;

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        caused_by_ethereal: bool,
    ) -> Flow {
        // `card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        if caused_by_ethereal {
            let ethereal_count = data(eng.s, me).ethereal_count + 1;
            eng.s
                .set_power_data(me.id, PowerData::DarkEmbrace(Data { ethereal_count }));
        } else {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.draw(amount, false)?;
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        eng.draw(
            Decimal::from(amount * data(eng.s, me).ethereal_count),
            false,
        )?;
        eng.s
            .set_power_data(me.id, PowerData::DarkEmbrace(Data::default()));
        Ok(())
    }
}
