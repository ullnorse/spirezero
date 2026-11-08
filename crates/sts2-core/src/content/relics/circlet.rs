//! `Circlet` (`Models.Relics/Circlet.cs`): `RelicRarity.None`, stackable, no effect. It is the
//! fallback (`RelicFactory.FallbackRelic`) when a relic bag has nothing left to give.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;

pub struct Circlet;

impl RelicModel for Circlet {
    fn name(&self) -> &'static str {
        "Circlet"
    }
}

#[sts2_macros::hooks]
impl Hooks for Circlet {
    type Me = RelicId;
}
