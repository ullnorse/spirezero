//! `MorphicGrove` (`Models.Events/MorphicGrove.cs`): lose all gold and transform 2 chosen cards,
//! or gain 5 max HP.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState};

/// `MaxHpVar(5m)`.
const MAX_HP: i32 = 5;
/// `_transformCount`.
const TRANSFORM_COUNT: usize = 2;
/// `IsAllowed`'s gold threshold.
const MIN_GOLD: i32 = 100;

pub struct MorphicGrove;

impl EventModel for MorphicGrove {
    /// At least 100 gold and 2 transformable deck cards (`IsTransformable` is `IsRemovable` in
    /// the deck; Quest cards count here).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= MIN_GOLD && s.deck.iter().filter(|c| c.is_removable()).count() >= TRANSFORM_COUNT
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("MORPHIC_GROVE.pages.INITIAL.options.GROUP", "GROUP"),
            EventOption::new("MORPHIC_GROVE.pages.INITIAL.options.LONER", "LONER"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "GROUP" => {
                let s = ctx.state();
                let gold = s.gold;
                s.lose_gold(gold);
                let options = s.transformable_cards();
                ctx.select_deck_cards(action, PromptText::Transform, options, TRANSFORM_COUNT)?;
            }
            _ => ctx.state().gain_max_hp(MAX_HP),
        }
        Ok(Page::finished("MORPHIC_GROVE", action.tag))
    }

    /// `Group` after `FromDeckForTransformation`: `TransformToRandom` on the event's `Rng`, per
    /// picked card in pick order.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        for (n, &i) in cards.iter().enumerate() {
            // Each transformed card left its place and its replacement joined the end.
            let shift = cards[..n].iter().filter(|&&j| j < i).count();
            ctx.transform_to_random(i - shift)?;
        }
        Ok(())
    }
}
