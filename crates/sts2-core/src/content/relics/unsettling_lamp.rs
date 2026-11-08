//! `UnsettlingLamp` (`Models.Relics/UnsettlingLamp.cs`): the first visible debuff a card applies to
//! an enemy each fight is doubled, together with every other debuff that same card applies.
//! `DoubledPowers` only matters through its `ITemporaryPower` members (the stat they apply
//! internally, which is not doubled again), and only its first entry can be set, because
//! `BeforePowerAmountChanged` returns once there is a triggering card.

use crate::combat::hooks::{Hooks, PowerQuery};
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, RelicId};
use crate::content::powers::{PowerKind, PowerType};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

pub struct UnsettlingLamp;

/// `UnsettlingLamp`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_triggeringCard`.
    pub triggering_card: Option<CardId>,
    /// The `InternallyAppliedPower` of the `ITemporaryPower` in `_doubledPowers`, if any.
    pub doubled_temporary_stat: Option<PowerKind>,
    /// `_isFinishedTriggering`.
    pub is_finished_triggering: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::UnsettlingLamp(data) = &mut s.relic_mut(me).data else {
        unreachable!("an UnsettlingLamp has UnsettlingLamp data");
    };
    data
}

fn data(s: &CombatState, me: RelicId) -> Data {
    let RelicData::UnsettlingLamp(data) = s.relic(me).data else {
        unreachable!("an UnsettlingLamp has UnsettlingLamp data");
    };
    data
}

impl RelicModel for UnsettlingLamp {
    fn name(&self) -> &'static str {
        "Unsettling Lamp"
    }

    fn init_data(&self) -> RelicData {
        RelicData::UnsettlingLamp(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for UnsettlingLamp {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }

    fn before_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        power: PowerQuery,
        amount: Decimal,
    ) -> Flow {
        let current = data(eng.s, me);
        if current.triggering_card.is_some() || current.is_finished_triggering {
            return Ok(());
        }
        let Some(card) = power.card else {
            return Ok(());
        };
        let player = eng.s.player.creature;
        // `target.Side == Owner.Creature.Side`.
        if power.applier != Some(player)
            || eng.s.creature(power.target).side == eng.s.creature(player).side
            || !power.kind.is_visible()
            || power.kind.type_for_amount(amount) != PowerType::Debuff
            || eng.s.creature(power.target).has_power(PowerKind::Artifact)
        {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.triggering_card = Some(card);
        data.doubled_temporary_stat = power.kind.internally_applied_power();
        Ok(())
    }

    fn modify_power_amount_given_multiplicative(
        &self,
        s: &CombatState,
        me: RelicId,
        power: PowerQuery,
        amount: Decimal,
    ) -> Decimal {
        let data = data(s, me);
        let Some(triggering) = data.triggering_card else {
            return Decimal::ONE;
        };
        if power.card != Some(triggering)
            || data.is_finished_triggering
            // `HasDoubledTemporaryPowerSource`.
            || data.doubled_temporary_stat == Some(power.kind)
            || power.kind.type_for_amount(amount) != PowerType::Debuff
        {
            return Decimal::ONE;
        }
        Decimal::from(2)
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        let data = data_mut(eng.s, me);
        if data.triggering_card == Some(play.card) && !data.is_finished_triggering {
            data.is_finished_triggering = true;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }
}
