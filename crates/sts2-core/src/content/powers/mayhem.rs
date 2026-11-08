//! `MayhemPower` (`Models.Powers/MayhemPower.cs`): when the auto-pre-play phase is entered
//! (after the hand is drawn), auto-play `Amount` cards from the top of the draw pile.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};

pub struct Mayhem;

impl PowerModel for Mayhem {
    fn may_prompt(&self) -> bool {
        // The auto-played cards can raise choices.
        true
    }

    fn name(&self) -> &'static str {
        "Mayhem"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Mayhem {
    type Me = PowerRef;

    fn after_auto_pre_play_phase_entered(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns it.
        let amount = eng.s.power(me.owner, me.id).amount;
        eng.auto_play_from_draw_pile(
            u32::try_from(amount).expect("Mayhem has a positive amount"),
            Position::Top,
            false,
        )
    }
}
