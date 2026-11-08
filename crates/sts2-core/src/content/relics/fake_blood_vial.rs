//! `FakeBloodVial` (`Models.Relics/FakeBloodVial.cs`): `FakeMerchant`'s `BloodVial`, heal 1 at the
//! start of the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `HealVar(1m)` (`IntValue`).
const HEAL: i32 = 1;

pub struct FakeBloodVial;

impl RelicModel for FakeBloodVial {
    fn name(&self) -> &'static str {
        "Blood Vial???"
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeBloodVial {
    type Me = RelicId;

    fn after_player_turn_start_late(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number <= 1 {
            let owner = eng.s.player.creature;
            eng.heal(owner, Decimal::from(HEAL))?;
        }
        Ok(())
    }
}
