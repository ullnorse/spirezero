//! `SicEmPower` (`Models.Powers/SicEmPower.cs`): a debuff; each time the applier's Osty hits the
//! owner, summon `Amount`. Removed at the end of the owner's side's turn.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SicEm;

impl PowerModel for SicEm {
    fn name(&self) -> &'static str {
        "Sic 'Em"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for SicEm {
    type Me = PowerRef;

    fn after_damage_given(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        // `dealer?.Monster is Osty` with a pet owner that is the power's applier.
        let power = eng.s.power(me.owner, me.id);
        if hit.dealer.is_none()
            || hit.dealer != eng.s.osty()
            || power.applier != Some(eng.s.player.creature)
            || hit.target != me.owner
        {
            return Ok(());
        }
        let amount = Decimal::from(power.amount);
        eng.summon_osty(amount)
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
