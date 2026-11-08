//! `Smog` (`Models.Afflictions/Smog.cs`): no behaviour of its own; `SmoggyPower` reads it to forbid
//! playing the card.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;

/// `SmoggyPower`'s affliction: the card cannot be played.
pub struct Smog;

impl AfflictionModel for Smog {
    fn name(&self) -> &'static str {
        "Smog"
    }
}

#[sts2_macros::hooks]
impl Hooks for Smog {
    type Me = CardId;
}
