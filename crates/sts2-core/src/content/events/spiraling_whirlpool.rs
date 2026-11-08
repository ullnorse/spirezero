//! `SpiralingWhirlpool` (`Models.Events/SpiralingWhirlpool.cs`): enchant a Basic Strike or
//! Defend with `Spiral`, or drink to heal a third of max HP.

use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunError, RunState, SelectPurpose};

/// `HealVar` factor: `MaxHp * 0.33m`.
const HEAL_FACTOR: Decimal = Decimal::new(33, 2);
/// `CardSelectorPrefs(EnchantSelectionPrompt, 1)`.
const PICKS: usize = 1;

pub struct SpiralingWhirlpool;

impl EventModel for SpiralingWhirlpool {
    /// Some deck card takes `Spiral`. A deck card that is not ported is no Basic Strike or
    /// Defend, so it never does.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.deck
            .iter()
            .any(|c| c.can_enchant(EnchantmentKind::Spiral).unwrap_or(false))
    }

    /// `CalculateVars`: `Heal = MaxHp * 0.33m`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let heal = Decimal::from(ctx.state().max_hp) * HEAL_FACTOR;
        *ctx.data = EventData::Amount(heal);
        Ok(vec![
            EventOption::new(
                "SPIRALING_WHIRLPOOL.pages.INITIAL.options.OBSERVE",
                "OBSERVE",
            ),
            EventOption::new("SPIRALING_WHIRLPOOL.pages.INITIAL.options.DRINK", "DRINK"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "OBSERVE" {
            let options = ctx.state().enchantable_cards(EnchantmentKind::Spiral)?;
            let purpose = SelectPurpose::Enchant {
                kind: EnchantmentKind::Spiral,
                amount: 1,
            };
            ctx.select(purpose, options, PICKS)?;
        } else {
            let EventData::Amount(heal) = *ctx.data else {
                unreachable!("SpiralingWhirlpool's heal is set in begin");
            };
            // `Heal(creature, Heal.IntValue)`.
            ctx.state().heal(Decimal::from(heal.to_int_truncating()));
        }
        Ok(Page::finished("SPIRALING_WHIRLPOOL", action.tag))
    }
}
