//! `MasterPlannerPower` (`Models.Powers/MasterPlannerPower.cs`): a buff (`Single`); every Skill the
//! owner plays gains Sly.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardType};
use crate::content::powers::{PowerModel, PowerType};

pub struct MasterPlanner;

impl PowerModel for MasterPlanner {
    fn name(&self) -> &'static str {
        "Master Planner"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for MasterPlanner {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner != Owner.Player`: every card is the only player's.
        if eng.s.card(play.card).card_type() == CardType::Skill {
            // Flash: visual only.
            eng.apply_keyword(play.card, CardKeyword::Sly);
        }
        Ok(())
    }
}
