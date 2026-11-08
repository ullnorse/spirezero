//! `Ringing` (`Models.Afflictions/Ringing.cs`): no behaviour of its own; `RingingPower` reads it
//! to allow one card play per turn. `HasExtraCardText` is UI only.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;

/// `RingingPower`'s affliction: one card play per turn.
pub struct Ringing;

impl AfflictionModel for Ringing {
    fn name(&self) -> &'static str {
        "Ringing"
    }
}

#[sts2_macros::hooks]
impl Hooks for Ringing {
    type Me = CardId;
}
