//! `PaelsWing` (`Models.Relics/PaelsWing.cs`): Ancient (Pael); every card reward gets a
//! `SACRIFICE` alternative (`TryModifyCardRewardAlternatives`) that completes the reward without
//! a card; every 2nd sacrifice (`RewardsSacrificed`) obtains `PullNextRelicFromFront`.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{CardAlternative, CardAlternatives, RunCtx, RunError, RunState};

/// `PaelsWing.sacrificeAlternativeKey`.
pub const SACRIFICE: &str = "SACRIFICE";
/// `DynamicVar("Sacrifices", 2m)`.
const SACRIFICES: i32 = 2;

pub struct PaelsWing;

/// `PaelsWing`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `RewardsSacrificed` (saved).
    pub rewards_sacrificed: i32,
}

impl RelicModel for PaelsWing {
    fn name(&self) -> &'static str {
        "Pael's Wing"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PaelsWing(Data::default())
    }

    fn try_modify_card_reward_alternatives(
        &self,
        _s: &RunState,
        me: usize,
        alternatives: &mut CardAlternatives,
    ) -> bool {
        alternatives.push(CardAlternative::Relic {
            relic: me,
            option: SACRIFICE,
        });
        true
    }

    /// `OnSacrifice`: `RewardsSacrificed++`, and on every 2nd `RelicCmd.Obtain` of
    /// `PullNextRelicFromFront(Owner)`.
    fn on_card_reward_alternative(
        &self,
        ctx: &mut RunCtx<'_>,
        me: usize,
        _option: &'static str,
    ) -> Result<(), RunError> {
        let RelicData::PaelsWing(data) = &mut ctx.state().relics[me].data else {
            unreachable!("a PaelsWing has PaelsWing data");
        };
        data.rewards_sacrificed += 1;
        if data.rewards_sacrificed % SACRIFICES != 0 {
            return Ok(());
        }
        let relic = ctx.pull_relic_from_front();
        ctx.obtain_relic(relic)
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsWing {
    type Me = RelicId;
}
