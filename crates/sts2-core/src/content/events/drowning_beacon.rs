//! `DrowningBeacon` (`Models.Events/DrowningBeacon.cs`): bottle a `GlowwaterPotion`, or lose 13
//! max HP for the `FresnelLens`.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{Reward, RunError};

/// `HpLossVar(13m)`.
const HP_LOSS: i32 = 13;

pub struct DrowningBeacon;

impl EventModel for DrowningBeacon {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("DROWNING_BEACON.pages.INITIAL.options.BOTTLE", "BOTTLE"),
            EventOption::new("DROWNING_BEACON.pages.INITIAL.options.CLIMB", "CLIMB"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "BOTTLE" {
            ctx.offer_rewards(vec![Reward::Potion("GLOWWATER_POTION")]);
        } else {
            ctx.state().lose_max_hp(HP_LOSS);
            ctx.obtain_relic("FRESNEL_LENS")?;
        }
        Ok(Page::finished("DROWNING_BEACON", action.tag))
    }
}
