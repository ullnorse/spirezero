//! `JungleMazeAdventure` (`Models.Events/JungleMazeAdventure.cs`): take 18 damage for about
//! 150 gold, or about 50 gold safely.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::RunError;
use crate::run::odds::decimal_from_f32;

/// `DamageVar("SoloHp", 18m, Unblockable | Unpowered)`.
const SOLO_HP: i32 = 18;

pub struct JungleMazeAdventure;

impl EventModel for JungleMazeAdventure {
    // `IsAllowed` is true in single player.

    /// `CalculateVars`: `SoloGold 150m` and `JoinForcesGold 50m` each `+= (decimal)
    /// Rng.NextFloat(-15f, 15f)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let solo_gold =
            Decimal::from(150) + decimal_from_f32(ctx.rng.next_float_range(-15.0, 15.0));
        let join_gold = Decimal::from(50) + decimal_from_f32(ctx.rng.next_float_range(-15.0, 15.0));
        *ctx.data = EventData::JungleMaze {
            solo_gold,
            join_gold,
        };
        Ok(vec![
            EventOption::new(
                "JUNGLE_MAZE_ADVENTURE.pages.INITIAL.options.SOLO_QUEST",
                "SOLO_QUEST",
            ),
            EventOption::new(
                "JUNGLE_MAZE_ADVENTURE.pages.INITIAL.options.JOIN_FORCES",
                "JOIN_FORCES",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let EventData::JungleMaze {
            solo_gold,
            join_gold,
        } = *ctx.data
        else {
            unreachable!("JungleMazeAdventure's vars are set in begin");
        };
        match action.tag {
            // `DontNeedHelp`: a `StableShuffle` of three VFX on the event `Rng` (nothing reads
            // that `Rng` afterwards), then the damage and the gold.
            "SOLO_QUEST" => {
                let mut fx = [0, 1, 2];
                crate::shuffle::stable_shuffle(&mut fx, ctx.rng);
                ctx.state().damage(SOLO_HP);
                ctx.state().gain_gold(solo_gold);
            }
            // `SafetyInNumbers`.
            _ => ctx.state().gain_gold(join_gold),
        }
        Ok(Page::finished("JUNGLE_MAZE_ADVENTURE", action.tag))
    }
}
