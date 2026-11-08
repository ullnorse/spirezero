//! `RuinedHelmet` (`Models.Relics/RuinedHelmet.cs`): the first positive Strength the owner
//! receives each combat is doubled.

use crate::combat::hooks::{Hooks, PowerQuery};
use crate::combat::{CombatState, Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

pub struct RuinedHelmet;

/// `RuinedHelmet`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_usedThisCombat`.
    pub used_this_combat: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::RuinedHelmet(data) = &mut s.relic_mut(me).data else {
        unreachable!("a RuinedHelmet has RuinedHelmet data");
    };
    data
}

impl RelicModel for RuinedHelmet {
    fn name(&self) -> &'static str {
        "Ruined Helmet"
    }

    fn init_data(&self) -> RelicData {
        RelicData::RuinedHelmet(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for RuinedHelmet {
    type Me = RelicId;

    fn try_modify_power_amount_received(
        &self,
        s: &CombatState,
        me: RelicId,
        power: PowerQuery,
        amount: Decimal,
    ) -> Option<Decimal> {
        let used = matches!(
            s.relic(me).data,
            RelicData::RuinedHelmet(Data {
                used_this_combat: true
            })
        );
        if power.kind != PowerKind::Strength
            || power.target != s.player.creature
            || amount <= Decimal::ZERO
            || used
        {
            return None;
        }
        Some(amount * Decimal::new(2, 0))
    }

    fn after_modifying_power_amount_received(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _power: PowerQuery,
    ) -> Flow {
        data_mut(eng.s, me).used_this_combat = true;
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).used_this_combat = false;
        Ok(())
    }
}
