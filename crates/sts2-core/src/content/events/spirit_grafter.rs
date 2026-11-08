//! `SpiritGrafter` (`Models.Events/SpiritGrafter.cs`): heal 25 and take a `Metamorphosis`, or
//! upgrade a card and take 10 damage.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError};

/// `HpLossVar("RejectionHpLoss", 10m)`.
const REJECTION_HP_LOSS: i32 = 10;
/// `HealVar("LetItInHealAmount", 25m)`.
const LET_IT_IN_HEAL: i32 = 25;
/// `CardSelectorPrefs(UpgradeSelectionPrompt, 1)`.
const UPGRADES: usize = 1;

pub struct SpiritGrafter;

impl EventModel for SpiritGrafter {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "SPIRIT_GRAFTER.pages.INITIAL.options.LET_IT_IN",
                "LET_IT_IN",
            ),
            EventOption::new(
                "SPIRIT_GRAFTER.pages.INITIAL.options.REJECTION",
                "REJECTION",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "LET_IT_IN" {
            let s = ctx.state();
            s.heal(Decimal::from(LET_IT_IN_HEAL));
            s.add_card(RunCard::new("METAMORPHOSIS"));
        } else {
            let options = ctx.state().upgradable_cards();
            ctx.select_deck_cards(action, PromptText::Upgrade, options, UPGRADES)?;
        }
        Ok(Page::finished("SPIRIT_GRAFTER", action.tag))
    }

    /// `Rejection` after `FromDeckForUpgrade` (no card if none is upgradable): `CardCmd.Upgrade`,
    /// then the unblockable damage.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let s = ctx.state();
        for &i in cards {
            s.deck[i].upgrade();
        }
        s.damage(REJECTION_HP_LOSS);
        Ok(())
    }
}
