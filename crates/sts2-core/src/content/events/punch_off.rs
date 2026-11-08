//! `PunchOff` (`Models.Events/PunchOff.cs`): take an `Injury` for a relic, or fight two
//! `PunchConstruct`s for a relic and a potion on top of the fight's rewards.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::rng::RunRngType;
use crate::run::{PendingReward, RunCard, RunError, RunState};

/// `IsAllowed`: `TotalFloor >= 6`.
const MIN_FLOOR: usize = 6;
/// The monsters of `PunchOffEventEncounter`.
const LAYOUT_MONSTERS: usize = 2;

pub struct PunchOff;

impl EventModel for PunchOff {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.total_floor() >= MIN_FLOOR
    }

    /// `EventCombatSynchronizer.InitializeForEvent` (the event has `EventLayoutType.Combat`)
    /// creates the fight's creatures when the event is entered, each rolling its unique max HP on
    /// `Rng.Niche` (`CombatState.CreateCreature`); the fight later reuses them. The engine
    /// creates the creatures when the fight starts, so the draws are made here and the stream
    /// is put back before `Fight` for the engine to make them again (the same values: nothing
    /// else draws `Niche` in between). Then `CalculateVars`: `Gold = Rng.NextInt(91, 99)`, a
    /// var no option reads.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let niche = &mut ctx.state().rng[RunRngType::Niche];
        let niche_before = niche.clone();
        for _ in 0..LAYOUT_MONSTERS {
            niche.next_int(1);
        }
        *ctx.data = EventData::CombatLayout { niche_before };
        ctx.rng.next_int_range(91, 99);
        Ok(vec![
            EventOption::new("PUNCH_OFF.pages.INITIAL.options.NAB", "NAB"),
            EventOption::new(
                "PUNCH_OFF.pages.INITIAL.options.I_CAN_TAKE_THEM",
                "TAKE_THEM",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "NAB" => {
                // `AddCurseToDeck<Injury>`, then `OfferCustom(RelicReward)`.
                ctx.state().add_card(RunCard::new("INJURY"));
                ctx.offer_pending_rewards(vec![PendingReward::Relic]);
                Ok(Page::finished("PUNCH_OFF", "NAB"))
            }
            "TAKE_THEM" => Ok(Page::Next(vec![EventOption::new(
                "PUNCH_OFF.pages.I_CAN_TAKE_THEM.options.FIGHT",
                "FIGHT",
            )])),
            _ => {
                // `EnterCombatWithoutExitingEvent<PunchOffEventEncounter>([Relic, Potion], false)`.
                let EventData::CombatLayout { niche_before } = ctx.data.clone() else {
                    unreachable!("PunchOff's Niche is saved in begin");
                };
                ctx.state().rng[RunRngType::Niche] = niche_before;
                ctx.enter_combat(
                    "PUNCH_OFF_EVENT_ENCOUNTER",
                    vec![PendingReward::Relic, PendingReward::Potion],
                    false,
                )?;
                Ok(Page::left())
            }
        }
    }
}
