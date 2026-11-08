//! `SwordOfStone` (`Models.Relics/SwordOfStone.cs`): after the 5th Elite fight won it turns into
//! `SwordOfJade` (`RelicCmd.Replace`, in `AfterCombatVictory`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{RoomType, RunCtx, RunError, RunState};

/// `DynamicVar("Elites", 5m)`.
const ELITES: i32 = 5;

pub struct SwordOfStone;

/// `SwordOfStone`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `ElitesDefeated`.
    pub elites_defeated: i32,
}

impl RelicModel for SwordOfStone {
    fn name(&self) -> &'static str {
        "Sword of Stone"
    }

    fn init_data(&self) -> RelicData {
        RelicData::SwordOfStone(Data::default())
    }

    /// The replacement (`SwordOfJade`, which may not be ported) comes with the 5th Elite.
    fn victory_in_run_may_fail(&self, s: &RunState, me: usize) -> bool {
        matches!(
            s.relics[me].data,
            RelicData::SwordOfStone(data) if data.elites_defeated + 1 >= ELITES
        )
    }

    fn after_combat_victory_in_run(
        &self,
        ctx: &mut RunCtx<'_>,
        me: usize,
        room: RoomType,
    ) -> Result<(), RunError> {
        if room != RoomType::Elite {
            return Ok(());
        }
        let RelicData::SwordOfStone(data) = &mut ctx.state().relics[me].data else {
            unreachable!("a SwordOfStone has SwordOfStone data");
        };
        data.elites_defeated += 1;
        if data.elites_defeated >= ELITES {
            ctx.replace_relic(me, "SWORD_OF_JADE")?;
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for SwordOfStone {
    type Me = RelicId;
}
