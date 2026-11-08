//! `Girya` (`Models.Relics/Girya.cs`): the Lift option at rest sites, up to 3 times
//! (`LiftRestSiteOption`); each lift is 1 Strength at the start of every later fight
//! (`AfterRoomEntered`). `IsAllowed`: before the Act 3 treasure.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RestOption, RestOptionKind, RunState};

/// `maxLifts`.
const MAX_LIFTS: i32 = 3;

pub struct Girya;

/// `Girya`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TimesLifted`.
    pub times_lifted: i32,
}

/// `LiftRestSiteOption.OnSelect`: `TimesLifted++`.
pub fn lift(s: &mut RunState, me: usize) {
    let RelicData::Girya(data) = &mut s.relics[me].data else {
        unreachable!("a Girya has Girya data");
    };
    data.times_lifted += 1;
}

/// `LiftRestSiteOption.Description`'s `LiftsLeft`: `3 - TimesLifted` of the relic at `me`.
#[must_use]
pub fn lifts_left(s: &RunState, me: usize) -> i32 {
    let RelicData::Girya(data) = s.relics[me].data else {
        unreachable!("a Girya has Girya data");
    };
    MAX_LIFTS - data.times_lifted
}

impl RelicModel for Girya {
    /// `IsAllowed` (`Girya.cs:45-48`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Girya"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Girya(Data::default())
    }

    fn try_modify_rest_site_options(
        &self,
        s: &RunState,
        me: usize,
        options: &mut Vec<RestOption>,
    ) -> bool {
        let RelicData::Girya(data) = s.relics[me].data else {
            unreachable!("a Girya has Girya data");
        };
        if data.times_lifted >= MAX_LIFTS {
            return false;
        }
        options.push(RestOption {
            kind: RestOptionKind::Lift,
            enabled: true,
        });
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Girya {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room.
        let RelicData::Girya(data) = eng.s.relic(me).data else {
            unreachable!("a Girya has Girya data");
        };
        if data.times_lifted > 0 {
            let owner = eng.s.player.creature;
            eng.apply_power(
                owner,
                PowerKind::Strength,
                Decimal::from(data.times_lifted),
                Some(owner),
            )?;
        }
        Ok(())
    }
}
