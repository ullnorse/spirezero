//! `FakeSneckoEye` (`Models.Relics/FakeSneckoEye.cs`): `FakeMerchant`'s `SneckoEye`, applies 1
//! `ConfusedPower` to the owner before combat starts. `AfterObtained` applies it too while a
//! combat is in progress, which never happens: relics are obtained outside fights. The test-mode
//! cost override is not modelled.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct FakeSneckoEye;

impl RelicModel for FakeSneckoEye {
    fn name(&self) -> &'static str {
        "Snecko Eye???"
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeSneckoEye {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        eng.apply_power(owner, PowerKind::Confused, Decimal::ONE, Some(owner))?;
        Ok(())
    }
}
