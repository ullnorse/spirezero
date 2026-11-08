//! `SpectrumShiftPower` (`Models.Powers/SpectrumShiftPower.cs`): a counter buff; before its
//! owner's hand draw, `Amount` distinct random Colorless cards are created in the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType};
use crate::content::cards::pools;
use crate::content::powers::{PowerModel, PowerType};

pub struct SpectrumShift;

impl PowerModel for SpectrumShift {
    fn may_prompt(&self) -> bool {
        // Generated Colorless cards prompt (Discovery, ...).
        true
    }

    fn name(&self) -> &'static str {
        "Spectrum Shift"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SpectrumShift {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        let cards = eng.get_distinct_for_combat(&pools::colorless(), amount)?;
        // Flash: visual only.
        eng.add_generated_cards_to_combat(&cards, PileType::Hand, Some(me.owner), Position::Bottom)
    }
}
