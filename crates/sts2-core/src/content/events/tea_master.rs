//! `TeaMaster` (`Models.Events/TeaMaster.cs`): buy `BoneTea` (50 gold), `EmberTea` (150 gold) or
//! take `TeaOfDiscourtesy`.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState};

/// `_boneTeaCost`.
const BONE_TEA_COST: i32 = 50;
/// `_emberTeaCost`.
const EMBER_TEA_COST: i32 = 150;

pub struct TeaMaster;

impl EventModel for TeaMaster {
    /// Before Act 3, with at least 150 gold.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index < 2 && s.gold >= EMBER_TEA_COST
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = ctx.state().gold;
        let bone = if gold >= BONE_TEA_COST {
            EventOption::new("TEA_MASTER.pages.INITIAL.options.BONE_TEA", "BONE_TEA")
        } else {
            EventOption::locked("TEA_MASTER.pages.INITIAL.options.BONE_TEA_LOCKED")
        };
        let ember = if gold >= EMBER_TEA_COST {
            EventOption::new("TEA_MASTER.pages.INITIAL.options.EMBER_TEA", "EMBER_TEA")
        } else {
            EventOption::locked("TEA_MASTER.pages.INITIAL.options.EMBER_TEA_LOCKED")
        };
        Ok(vec![
            bone,
            ember,
            EventOption::new(
                "TEA_MASTER.pages.INITIAL.options.TEA_OF_DISCOURTESY",
                "TEA_OF_DISCOURTESY",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "BONE_TEA" => {
                ctx.state().lose_gold(BONE_TEA_COST);
                ctx.obtain_relic("BONE_TEA")?;
            }
            "EMBER_TEA" => {
                ctx.state().lose_gold(EMBER_TEA_COST);
                ctx.obtain_relic("EMBER_TEA")?;
            }
            _ => ctx.obtain_relic("TEA_OF_DISCOURTESY")?,
        }
        let page = if action.tag == "TEA_OF_DISCOURTESY" {
            "TEA_OF_DISCOURTESY"
        } else {
            "DONE"
        };
        Ok(Page::finished("TEA_MASTER", page))
    }
}
