//! `AbyssalBaths` (`Models.Events/AbyssalBaths.cs`): immerse for max HP at a growing unblockable
//! cost, as often as the player dares, or abstain and heal.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::RunError;

/// `MaxHpVar(2m)`.
const MAX_HP: i32 = 2;
/// `DamageVar(3m, Unblockable | Unpowered)`, the starting value (`_baseDamage`).
const BASE_DAMAGE: i32 = 3;
/// `HealVar(10m)`.
const HEAL: i32 = 10;

pub struct AbyssalBaths;

/// The current `Damage` var.
fn damage(ctx: &EventCtx<'_>) -> Decimal {
    let EventData::Amount(damage) = *ctx.data else {
        unreachable!("AbyssalBaths' damage is set in begin");
    };
    damage
}

/// `OnImmerse`: `GainMaxHp`, `Damage` by the var, then the var goes up by 1.
fn on_immerse(ctx: &mut EventCtx<'_>) {
    let current = damage(ctx);
    let s = ctx.state();
    s.gain_max_hp(MAX_HP);
    s.damage(current.to_int_truncating());
    *ctx.data = EventData::Amount(current + Decimal::ONE);
}

/// The page after an immersion (`Immerse`, `Linger`): `Linger` again or `ExitBaths`. The
/// `DEATH_WARNING` and `LINGERn` descriptions are text only.
fn next_page() -> Page {
    Page::Next(vec![
        EventOption::new("ABYSSAL_BATHS.pages.ALL.options.LINGER", "LINGER"),
        EventOption::new("ABYSSAL_BATHS.pages.ALL.options.EXIT_BATHS", "EXIT_BATHS"),
    ])
}

impl EventModel for AbyssalBaths {
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        *ctx.data = EventData::Amount(Decimal::from(BASE_DAMAGE));
        Ok(vec![
            EventOption::new("ABYSSAL_BATHS.pages.INITIAL.options.IMMERSE", "IMMERSE"),
            EventOption::new("ABYSSAL_BATHS.pages.INITIAL.options.ABSTAIN", "ABSTAIN"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "IMMERSE" | "LINGER" => {
                on_immerse(ctx);
                Ok(next_page())
            }
            "ABSTAIN" => {
                ctx.state().heal(Decimal::from(HEAL));
                Ok(Page::finished("ABYSSAL_BATHS", action.tag))
            }
            _ => Ok(Page::finished("ABYSSAL_BATHS", action.tag)),
        }
    }
}
