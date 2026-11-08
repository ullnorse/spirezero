//! `Entangled` (`Models.Afflictions/Entangled.cs`): no behaviour of its own; `TangledPower` reads
//! it to raise the card's cost.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;

/// `TangledPower`'s affliction: the card costs `Tangled`'s amount more.
pub struct Entangled;

impl AfflictionModel for Entangled {
    fn name(&self) -> &'static str {
        "Entangled"
    }
}

#[sts2_macros::hooks]
impl Hooks for Entangled {
    type Me = CardId;
}
