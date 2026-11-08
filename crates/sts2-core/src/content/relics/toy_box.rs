//! `ToyBox` (`Models.Relics/ToyBox.cs`): Ancient (Tezcatara); `AfterObtained` offers 5 wax relics
//! (`PullNextRelicFromFront`, `IsWax`) as custom rewards; every 3rd combat (`CombatsSeen`, up to
//! 15) melts the first unmelted wax relic (`RelicCmd.Melt`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{Reward, RoomType, RunCtx, RunError, RunState};

/// `DynamicVar("Relics", 5m)`.
const RELICS: i32 = 5;
/// `DynamicVar("Combats", 3m)`.
const COMBATS: i32 = 3;

pub struct ToyBox;

/// `ToyBox`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CombatsSeen` (saved).
    pub combats_seen: i32,
}

fn data(d: &RelicData) -> Data {
    let RelicData::ToyBox(d) = *d else {
        unreachable!("a ToyBox has ToyBox data");
    };
    d
}

impl RelicModel for ToyBox {
    fn name(&self) -> &'static str {
        "Toy Box"
    }

    fn init_data(&self) -> RelicData {
        RelicData::ToyBox(Data::default())
    }

    /// `CombatsSeen >= Combats * Relics`.
    fn is_used_up(&self, d: &RelicData) -> bool {
        data(d).combats_seen >= COMBATS * RELICS
    }

    /// `RewardsCmd.OfferCustom` of 5 `RelicReward`s, each pulled (`PullNextRelicFromFront`) and
    /// made wax in turn.
    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let rewards = (0..RELICS)
            .map(|_| Reward::WaxRelic(ctx.pull_relic_from_front()))
            .collect();
        ctx.offer_rewards(rewards);
        Ok(())
    }

    /// `AfterCombatEnd` of any combat room: unless used up, `CombatsSeen++`, and every 3rd melts
    /// the first owned wax relic not yet melted. It only touches the run's relics, so the run
    /// runs it once the fight is over.
    fn after_combat_end_on_deck(&self, s: &mut RunState, me: usize, _room: RoomType) {
        if self.is_used_up(&s.relics[me].data) {
            return;
        }
        let RelicData::ToyBox(d) = &mut s.relics[me].data else {
            unreachable!("a ToyBox has ToyBox data");
        };
        d.combats_seen += 1;
        if d.combats_seen % COMBATS == 0
            && let Some(i) = s.relics.iter().position(|r| r.wax && !r.melted)
        {
            s.melt_relic(i);
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for ToyBox {
    type Me = RelicId;
}
