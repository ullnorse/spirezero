//! `NemesisPower` (`Models.Powers/NemesisPower.cs`): a buff (`Single`) on `TestSubject`'s third
//! form. At the end of every turn of its owner's side it alternates: gain 1 `Intangible`, then
//! lose its `Intangible`, starting with the gain.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Nemesis;

/// `NemesisPower`'s field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_shouldApplyIntangible`.
    pub should_apply_intangible: bool,
}

fn should_apply_intangible(s: &CombatState, me: PowerRef) -> bool {
    match *s.power_data(me.id) {
        PowerData::Nemesis(data) => data.should_apply_intangible,
        _ => unreachable!("a Nemesis instance holds Nemesis data"),
    }
}

impl PowerModel for Nemesis {
    fn name(&self) -> &'static str {
        "Nemesis"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Nemesis(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Nemesis {
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
        let apply = !should_apply_intangible(eng.s, me);
        eng.s.set_power_data(
            me.id,
            PowerData::Nemesis(Data {
                should_apply_intangible: apply,
            }),
        );
        if apply {
            eng.apply_power(
                me.owner,
                PowerKind::Intangible,
                Decimal::ONE,
                Some(me.owner),
            )?;
        } else if let Some(intangible) = eng
            .s
            .creature(me.owner)
            .powers
            .iter()
            .find(|p| p.kind == PowerKind::Intangible)
            .map(|p| p.id)
        {
            eng.remove_power(me.owner, intangible)?;
        }
        Ok(())
    }
}
