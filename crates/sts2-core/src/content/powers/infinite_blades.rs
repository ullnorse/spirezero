//! `InfiniteBladesPower` (`Models.Powers/InfiniteBladesPower.cs`): a counter buff; before its
//! owner's hand draw, `Amount` Shivs are created in the hand.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::cards::shiv;
use crate::content::powers::{PowerModel, PowerType};

pub struct InfiniteBlades;

impl PowerModel for InfiniteBlades {
    fn name(&self) -> &'static str {
        "Infinite Blades"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for InfiniteBlades {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        // Flash: visual only.
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        shiv::create_in_hand(eng, amount)?;
        Ok(())
    }
}
