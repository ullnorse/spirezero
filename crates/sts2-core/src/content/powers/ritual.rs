//! `RitualPower` (`Models.Powers/RitualPower.cs`): at the end of its side's turn the owner gains
//! `Amount` Strength, except at the end of the turn right after an enemy owner received it.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Ritual;

/// `RitualPower._wasJustAppliedByEnemy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub was_just_applied_by_enemy: bool,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Ritual(data) => data,
        _ => unreachable!("a Ritual instance holds Ritual data"),
    }
}

impl PowerModel for Ritual {
    fn name(&self) -> &'static str {
        "Ritual"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Ritual(Data::default())
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        if eng.s.creature(me.owner).side == Side::Enemy {
            eng.s.set_power_data(
                me.id,
                PowerData::Ritual(Data {
                    was_just_applied_by_enemy: true,
                }),
            );
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Ritual {
    type Me = PowerRef;

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
        if data(eng.s, me).was_just_applied_by_enemy {
            eng.s
                .set_power_data(me.id, PowerData::Ritual(Data::default()));
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.apply_power(me.owner, PowerKind::Strength, amount, Some(me.owner))
    }
}
