//! `RoyaltiesPower` (`Models.Powers/RoyaltiesPower.cs`): a buff (`Counter`); when a combat is won
//! (`AfterCombatEnd`), the room offers an extra `GoldReward(Amount)`
//! (`CombatRoom.AddExtraReward`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, ExtraReward, Flow};
use crate::content::powers::{PowerModel, PowerType};

pub struct Royalties;

impl PowerModel for Royalties {
    fn name(&self) -> &'static str {
        "Royalties"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Royalties {
    type Me = PowerRef;

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        let amount = eng.s.power(me.owner, me.id).amount;
        eng.s.extra_rewards.push(ExtraReward::Gold(amount));
        Ok(())
    }
}
