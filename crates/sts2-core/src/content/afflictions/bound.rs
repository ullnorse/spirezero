//! `Bound` (`Models.Afflictions/Bound.cs`): no behaviour of its own; `ChainsOfBindingPower` reads
//! it to allow one Bound card play per turn. `HasExtraCardText` is UI only.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;

/// `ChainsOfBindingPower`'s affliction: one Bound card play per turn.
pub struct Bound;

impl AfflictionModel for Bound {
    fn name(&self) -> &'static str {
        "Bound"
    }
}

#[sts2_macros::hooks]
impl Hooks for Bound {
    type Me = CardId;
}
