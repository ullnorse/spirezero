//! `WhisperingHollow` (`Models.Events/WhisperingHollow.cs`): pay about 35 gold for 2 potion
//! rewards, or transform a chosen card at 9 HP.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{PendingReward, RunError, RunState};

/// `_baseGold`.
const BASE_GOLD: i32 = 35;
/// `_goldVariance`.
const GOLD_VARIANCE: i32 = 9;
/// `HpLossVar(9m)`.
const HP_LOSS: i32 = 9;
/// `IsAllowed`'s gold threshold.
const MIN_GOLD: i32 = 44;
/// `CardSelectorPrefs(TransformSelectionPrompt, 1)`.
const TRANSFORMS: usize = 1;

pub struct WhisperingHollow;

impl EventModel for WhisperingHollow {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= MIN_GOLD
    }

    /// `CalculateVars`: `Gold += Rng.NextInt(-9, 10)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = BASE_GOLD + ctx.rng.next_int_range(-GOLD_VARIANCE, GOLD_VARIANCE + 1);
        *ctx.data = EventData::Amount(Decimal::from(gold));
        Ok(vec![
            EventOption::new("WHISPERING_HOLLOW.pages.INITIAL.options.GOLD", "GOLD"),
            EventOption::new("WHISPERING_HOLLOW.pages.INITIAL.options.HUG", "HUG"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "GOLD" {
            let EventData::Amount(gold) = *ctx.data else {
                unreachable!("WhisperingHollow's gold is set in begin");
            };
            ctx.state().lose_gold(gold.to_int_truncating());
            ctx.offer_pending_rewards(vec![PendingReward::Potion, PendingReward::Potion]);
        } else {
            let options = ctx.state().transformable_cards();
            ctx.select_deck_cards(action, PromptText::Transform, options, TRANSFORMS)?;
        }
        Ok(Page::finished("WHISPERING_HOLLOW", action.tag))
    }

    /// `Hug` after `FromDeckForTransformation`: `TransformToRandom` on the event's `Rng`, then
    /// the unblockable damage.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        for &i in cards {
            ctx.transform_to_random(i)?;
        }
        ctx.state().damage(HP_LOSS);
        Ok(())
    }
}
