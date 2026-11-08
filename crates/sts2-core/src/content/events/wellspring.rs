//! `Wellspring` (`Models.Events/Wellspring.cs`): a potion reward drawn uniformly from every
//! potion, or remove a card and take a `Guilty`.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::rng::PlayerRngType;
use crate::run::potions::potion_options;
use crate::run::{Reward, RunCard, RunError};

/// `DynamicVar("BatheCurses", 1m)`.
const BATHE_CURSES: usize = 1;
/// `CardSelectorPrefs(RemoveSelectionPrompt, 1)`.
const BATHE_REMOVALS: usize = 1;

pub struct Wellspring;

impl EventModel for Wellspring {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("WELLSPRING.pages.INITIAL.options.BOTTLE", "BOTTLE"),
            EventOption::new("WELLSPRING.pages.INITIAL.options.BATHE", "BATHE"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "BOTTLE" {
            // A single `NextItem` on the player's `rewards` over the character pool then the
            // shared pool, not the rarity-weighted `PotionFactory`.
            let potions: Vec<&'static str> = potion_options(ctx.state().character)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            let rng = &mut ctx.state().player_rng[PlayerRngType::Rewards];
            let potion = *rng
                .next_item(&potions)
                .expect("the potion pools are not empty");
            ctx.offer_rewards(vec![Reward::Potion(potion)]);
        } else {
            let options = ctx.state().removable_cards();
            ctx.select_deck_cards(action, PromptText::Remove, options, BATHE_REMOVALS)?;
        }
        Ok(Page::finished("WELLSPRING", action.tag))
    }

    /// `Bathe` after `FromDeckForRemoval`: `RemoveFromDeck`, then `AddCursesToDeck(Guilty)`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let s = ctx.state();
        s.remove_cards(cards);
        for _ in 0..BATHE_CURSES {
            s.add_card(RunCard::new("GUILTY"));
        }
        Ok(())
    }
}
