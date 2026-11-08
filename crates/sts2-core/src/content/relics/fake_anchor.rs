//! `FakeAnchor` (`Models.Relics/FakeAnchor.cs`): `FakeMerchant`'s `Anchor`, 4 Block before combat
//! starts.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(4m, ValueProp.Unpowered)`.
const BLOCK: i32 = 4;

pub struct FakeAnchor;

impl RelicModel for FakeAnchor {
    fn name(&self) -> &'static str {
        "Anchor???"
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeAnchor {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
