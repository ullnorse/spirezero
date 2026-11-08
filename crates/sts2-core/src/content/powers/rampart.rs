//! `RampartPower` (`Models.Powers/RampartPower.cs`): a buff (`Counter`) on `LivingShield`. When the
//! player's side turn starts (not an extra turn), every `TurretOperator` gains `Amount` Block.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::monsters::MonsterKind;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Rampart;

impl PowerModel for Rampart {
    fn name(&self) -> &'static str {
        "Rampart"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Rampart {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side != Side::Player || eng.s.player.extra_turn {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        let operators: Vec<CreatureId> = eng
            .s
            .enemies
            .iter()
            .copied()
            .filter(|&e| {
                matches!(
                    eng.s.creature(e).monster.as_ref().map(|m| m.kind),
                    Some(MonsterKind::TurretOperator(_))
                )
            })
            .collect();
        for operator in operators {
            eng.gain_block(operator, amount, ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
