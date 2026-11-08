//! `SeekingEdgePower` (`Models.Powers/SeekingEdgePower.cs`): a buff (`Single`) with no hooks;
//! its owner's Sovereign Blades target every enemy (`SovereignBlade.TargetType`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct SeekingEdge;

impl PowerModel for SeekingEdge {
    fn name(&self) -> &'static str {
        "Seeking Edge"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SeekingEdge {
    type Me = PowerRef;
}
