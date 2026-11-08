//! `Galvanized` (`Models.Afflictions/Galvanized.cs`): stackable, no behaviour of its own;
//! `GalvanicPower` reads it to hurt the player when the card is played.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;

/// `GalvanicPower`'s affliction on Power cards.
pub struct Galvanized;

impl AfflictionModel for Galvanized {
    fn name(&self) -> &'static str {
        "Galvanized"
    }

    fn is_stackable(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Galvanized {
    type Me = CardId;
}
