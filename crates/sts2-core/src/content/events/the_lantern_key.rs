//! `TheLanternKey` (`Models.Events/TheLanternKey.cs`): return the key for 100 gold, or keep it and
//! fight the `MysteriousKnight` for a `LanternKey` (a `SpecialCardReward` on top of the fight's
//! rewards; the run goes back to the map after them).

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::rng::RunRngType;
use crate::run::{PendingReward, Reward, RunCard, RunError};

/// `GoldVar(100)`.
const GOLD: i32 = 100;

pub struct TheLanternKey;

impl EventModel for TheLanternKey {
    /// `EventCombatSynchronizer.InitializeForEvent` (`EventLayoutType.Combat`) creates the
    /// `MysteriousKnight` when the event is entered, rolling its unique max HP on `Rng.Niche`
    /// (`CombatState.CreateCreature`, `CombatState.cs:241`); `Fight` reuses it. As for
    /// [`super::punch_off`], the draw is made here and the stream put back before the engine
    /// creates the knight.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let niche = &mut ctx.state().rng[RunRngType::Niche];
        let niche_before = niche.clone();
        niche.next_int(1);
        *ctx.data = EventData::CombatLayout { niche_before };
        Ok(vec![
            EventOption::new(
                "THE_LANTERN_KEY.pages.INITIAL.options.RETURN_THE_KEY",
                "RETURN_THE_KEY",
            ),
            EventOption::new(
                "THE_LANTERN_KEY.pages.INITIAL.options.KEEP_THE_KEY",
                "KEEP_THE_KEY",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "RETURN_THE_KEY" => {
                ctx.state().gain_gold(Decimal::from(GOLD));
                Ok(Page::Finished(
                    "THE_LANTERN_KEY.pages.DONE.options.RETURN_THE_KEY.description".to_owned(),
                ))
            }
            "KEEP_THE_KEY" => Ok(Page::Next(vec![EventOption::new(
                "THE_LANTERN_KEY.pages.KEEP_THE_KEY.options.FIGHT",
                "FIGHT",
            )])),
            _ => {
                let EventData::CombatLayout { niche_before } = ctx.data.clone() else {
                    unreachable!("TheLanternKey's Niche is saved in begin");
                };
                ctx.state().rng[RunRngType::Niche] = niche_before;
                let key = Reward::SpecialCard(RunCard::new("LANTERN_KEY"));
                ctx.enter_combat(
                    "MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER",
                    vec![PendingReward::Ready(key)],
                    false,
                )?;
                Ok(Page::left())
            }
        }
    }
}
