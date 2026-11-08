//! `SapphireSeed` (`Models.Events/SapphireSeed.cs`): heal and upgrade a card, or enchant a
//! card with `Sown`.

use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunError, SelectPurpose};

/// `HealVar(9m)`.
const HEAL: i32 = 9;
/// `CardSelectorPrefs(.., 1)`.
const PICKS: usize = 1;
/// `Sown` is enchanted with `1`.
const SOWN_AMOUNT: i32 = 1;

pub struct SapphireSeed;

impl EventModel for SapphireSeed {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("SAPPHIRE_SEED.pages.INITIAL.options.EAT", "EAT"),
            EventOption::new("SAPPHIRE_SEED.pages.INITIAL.options.PLANT", "PLANT"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "EAT" {
            ctx.state().heal(Decimal::from(HEAL));
            let options = ctx.state().upgradable_cards();
            ctx.select(SelectPurpose::Upgrade, options, PICKS)?;
        } else {
            let kind = EnchantmentKind::Sown;
            let options = ctx.state().enchantable_cards(kind)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: SOWN_AMOUNT,
            };
            ctx.select(purpose, options, PICKS)?;
        }
        Ok(Page::finished("SAPPHIRE_SEED", action.tag))
    }
}
