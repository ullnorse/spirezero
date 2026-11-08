//! `LuminousChoir` (`Models.Events/LuminousChoir.cs`): remove 2 chosen cards and take a
//! `SporeMind`, or pay about 149 gold for the next relic.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError, RunState};

/// `GoldVar(149)`.
const GOLD: i32 = 149;
/// `CardSelectorPrefs(RemoveSelectionPrompt, 2)`.
const REMOVALS: usize = 2;

pub struct LuminousChoir;

impl EventModel for LuminousChoir {
    /// The canonical `Gold` (149) and the player's relic bag having a relic left.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= GOLD && s.player_relic_bag.has_available_relics(s)
    }

    /// `CalculateVars`: `Gold -= Rng.NextInt(0, 50)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = GOLD - ctx.rng.next_int_range(0, 50);
        *ctx.data = EventData::Amount(Decimal::from(gold));
        let tribute = if ctx.state().gold >= gold {
            EventOption::new(
                "LUMINOUS_CHOIR.pages.INITIAL.options.OFFER_TRIBUTE",
                "OFFER_TRIBUTE",
            )
        } else {
            EventOption::locked("LUMINOUS_CHOIR.pages.INITIAL.options.OFFER_TRIBUTE_LOCKED")
        };
        Ok(vec![
            EventOption::new(
                "LUMINOUS_CHOIR.pages.INITIAL.options.REACH_INTO_THE_FLESH",
                "REACH_INTO_THE_FLESH",
            ),
            tribute,
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "REACH_INTO_THE_FLESH" {
            let options = ctx.state().removable_cards();
            ctx.select_deck_cards(action, PromptText::Remove, options, REMOVALS)?;
        } else {
            let EventData::Amount(gold) = *ctx.data else {
                unreachable!("LuminousChoir's gold is set in begin");
            };
            ctx.state().lose_gold(gold.to_int_truncating());
            let relic = ctx.pull_relic_from_front();
            ctx.obtain_relic(relic)?;
        }
        Ok(Page::finished("LUMINOUS_CHOIR", action.tag))
    }

    /// `ReachIntoTheFlesh` after `FromDeckForRemoval`: `RemoveFromDeck`, then
    /// `AddCurseToDeck<SporeMind>`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let s = ctx.state();
        s.remove_cards(cards);
        s.add_card(RunCard::new("SPORE_MIND"));
        Ok(())
    }
}
