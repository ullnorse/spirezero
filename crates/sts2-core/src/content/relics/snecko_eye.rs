//! `SneckoEye` (`Models.Relics/SneckoEye.cs`): applies 1 `ConfusedPower` to the owner before
//! combat starts, and the owner draws 2 more cards each hand draw. `AfterObtained` applies the
//! power too while a combat is in progress, which never happens: relics are obtained outside
//! fights. The test-mode cost override is not modelled.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(2)`.
const CARDS: Decimal = Decimal::new(2, 0);

pub struct SneckoEye;

impl RelicModel for SneckoEye {
    fn name(&self) -> &'static str {
        "Snecko Eye"
    }
}

#[sts2_macros::hooks]
impl Hooks for SneckoEye {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        eng.apply_power(owner, PowerKind::Confused, Decimal::ONE, Some(owner))?;
        Ok(())
    }

    fn modify_hand_draw(&self, _s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        // The only player is the owner.
        count + CARDS
    }
}
