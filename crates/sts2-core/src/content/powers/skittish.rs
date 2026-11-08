//! `SkittishPower` (`Models.Powers/SkittishPower.cs`): the first time each turn a card's attack
//! takes HP off the owner, it gains `Amount` unpowered block. The flag resets when the other side's
//! turn ends. `ShouldScaleInMultiplayer` only matters with several players.

use crate::combat::commands::Attack;
use crate::combat::commands::damage::DamageResult;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Skittish;

/// `SkittishPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `hasGainedBlockThisTurn`.
    pub has_gained_block_this_turn: bool,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Skittish(data) => data,
        _ => unreachable!("a Skittish instance holds Skittish data"),
    }
}

fn set_gained(eng: &mut Engine<'_>, me: PowerRef, value: bool) {
    eng.s.set_power_data(
        me.id,
        PowerData::Skittish(Data {
            has_gained_block_this_turn: value,
        }),
    );
}

impl PowerModel for Skittish {
    fn name(&self) -> &'static str {
        "Skittish"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Skittish(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Skittish {
    type Me = PowerRef;

    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        attack: &Attack,
        results: &[DamageResult],
    ) -> Flow {
        if data(eng.s, me).has_gained_block_this_turn
            || !attack.props.has(ValueProps::MOVE)
            || attack.card.is_none()
        {
            return Ok(());
        }
        // `FirstOrDefault(r => r.Receiver == Owner)`: only the owner's first result counts.
        let hit = results.iter().find(|r| r.receiver == me.owner);
        if hit.is_some_and(|r| r.unblocked != 0) {
            set_gained(eng, me, true);
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side != eng.s.creature(me.owner).side {
            set_gained(eng, me, false);
        }
        Ok(())
    }
}
