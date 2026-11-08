//! `RoundTeaParty` (`Models.Events/RoundTeaParty.cs`): enjoy the tea for the `RoyalPoison` relic
//! and a full heal, or pick a fight: lose 11 HP for a relic from the bag. Needs 12 HP.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::content::relics::RelicKind;
use crate::decimal::Decimal;
use crate::run::{RunError, RunState};

/// `DamageVar(11m, Unblockable | Unpowered)`.
const DAMAGE: i32 = 11;

pub struct RoundTeaParty;

impl EventModel for RoundTeaParty {
    /// `Players.All(p => p.Creature.CurrentHp >= 12)`: more than the 11 damage of Pick Fight.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.hp > DAMAGE
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "ROUND_TEA_PARTY.pages.INITIAL.options.ENJOY_TEA",
                "ENJOY_TEA",
            ),
            EventOption::new(
                "ROUND_TEA_PARTY.pages.INITIAL.options.PICK_FIGHT",
                "PICK_FIGHT",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "ENJOY_TEA" => {
                ctx.obtain_relic(RelicKind::RoyalPoison.id())?;
                let s = ctx.state();
                s.heal(Decimal::from(s.max_hp - s.hp));
                Ok(Page::finished("ROUND_TEA_PARTY", "ENJOY_TEA"))
            }
            "PICK_FIGHT" => Ok(Page::Next(vec![EventOption::new(
                "ROUND_TEA_PARTY.pages.PICK_FIGHT.options.CONTINUE_FIGHT",
                "CONTINUE_FIGHT",
            )])),
            _ => {
                ctx.state().damage(DAMAGE);
                let relic = ctx.pull_relic_from_front();
                ctx.obtain_relic(relic)?;
                Ok(Page::finished("ROUND_TEA_PARTY", "CONTINUE_FIGHT"))
            }
        }
    }
}
