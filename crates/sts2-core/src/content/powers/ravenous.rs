//! `RavenousPower` (`Models.Powers/RavenousPower.cs`): when another creature on the owner's side
//! dies while the owner lives, the owner (a `CorpseSlug`) is stunned and gains `Amount` Strength.
//! The stun move only resets `CorpseSlug.IsRavenous` and plays an animation, so it is `STUNNED`
//! (back into the move it last used).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::monsters;
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Ravenous;

impl PowerModel for Ravenous {
    fn name(&self) -> &'static str {
        "Ravenous"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Ravenous {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        let owner = eng.s.creature(me.owner);
        if was_removal_prevented
            || creature == me.owner
            || eng.s.creature(creature).side != owner.side
            || owner.is_dead()
        {
            return Ok(());
        }
        // `CorpseSlug.IsRavenous = true` only drives the animation.
        monsters::stun(eng, me.owner, None);
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.apply_power(me.owner, PowerKind::Strength, amount, Some(me.owner))
    }
}
