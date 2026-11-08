//! `PumpkinCandle` (`Models.Relics/PumpkinCandle.cs`): Ancient (Tezcatara); while `KindleCount`
//! is positive, max energy is 1 higher. Obtaining it and the `KINDLE` rest-site option
//! (`KindleRestSiteOption`) each add 5 (`Rekindle`); every combat end takes 1 off (not below 0).
//! `Status` is display only.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::run::{RestOption, RestOptionKind, RunCtx, RunError, RunState};

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `kindleAmount`.
pub const KINDLE_AMOUNT: i32 = 5;

pub struct PumpkinCandle;

/// `PumpkinCandle`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `KindleCount`.
    pub kindle_count: i32,
}

/// `Rekindle` on the relic at `me` of `s.relics`: `KindleCount += 5`.
pub fn rekindle(s: &mut RunState, me: usize) {
    let RelicData::PumpkinCandle(data) = &mut s.relics[me].data else {
        unreachable!("a PumpkinCandle has PumpkinCandle data");
    };
    data.kindle_count += KINDLE_AMOUNT;
}

impl RelicModel for PumpkinCandle {
    fn name(&self) -> &'static str {
        "Pumpkin Candle"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PumpkinCandle(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        rekindle(ctx.state(), me);
        Ok(())
    }

    fn try_modify_rest_site_options(
        &self,
        _s: &RunState,
        _me: usize,
        options: &mut Vec<RestOption>,
    ) -> bool {
        options.push(RestOption {
            kind: RestOptionKind::Kindle,
            enabled: true,
        });
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for PumpkinCandle {
    type Me = RelicId;

    fn modify_max_energy(&self, s: &CombatState, me: RelicId, amount: Decimal) -> Decimal {
        let RelicData::PumpkinCandle(data) = s.relic(me).data else {
            unreachable!("a PumpkinCandle has PumpkinCandle data");
        };
        if data.kindle_count <= 0 {
            return amount;
        }
        amount + ENERGY
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::PumpkinCandle(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a PumpkinCandle has PumpkinCandle data");
        };
        data.kindle_count = (data.kindle_count - 1).max(0);
        Ok(())
    }
}
