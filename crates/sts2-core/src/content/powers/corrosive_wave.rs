//! `CorrosiveWavePower` (`Models.Powers/CorrosiveWavePower.cs`): each card the owner draws
//! gives `Amount` Poison to every hittable enemy; removed at the end of the owner's side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct CorrosiveWave;

impl PowerModel for CorrosiveWave {
    fn name(&self) -> &'static str {
        "Corrosive Wave"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for CorrosiveWave {
    type Me = PowerRef;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        let enemies = eng.hooked().hittable_enemies();
        eng.apply_power_to_all(&enemies, PowerKind::Poison, amount, Some(me.owner))
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
