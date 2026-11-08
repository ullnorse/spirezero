//! `StoneOfAllTime` (`Models.Events/StoneOfAllTime.cs`): drink a random potion for 10 max HP, or
//! lose 6 HP to enchant an attack with `Vigorous` 8. Shared, Act 2 only; the belt is locked while
//! it runs (`CanUseOrRemovePotions`).

use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState, SelectPurpose};

/// `DynamicVar("DrinkMaxHpGain", 10m)`.
const DRINK_MAX_HP_GAIN: i32 = 10;
/// `DynamicVar("PushHpLoss", 6m)`, unblockable and unpowered.
const PUSH_HP_LOSS: i32 = 6;
/// `DynamicVar("PushVigorousAmount", 8m)`.
const PUSH_VIGOROUS_AMOUNT: i32 = 8;
/// `CardSelectorPrefs(EnchantSelectionPrompt, 1)`.
const ENCHANTS: usize = 1;
/// Both options end with a discarded `Rng.NextInt(100)`.
const UNUSED_ROLL: i32 = 100;

pub struct StoneOfAllTime;

impl EventModel for StoneOfAllTime {
    /// `CurrentActIndex == 1` and a potion held (`StoneOfAllTime.cs:55-62`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index == 1 && s.potions_held().next().is_some()
    }

    fn locks_potions(&self) -> bool {
        true
    }

    /// `DrinkAndLiftPotion = Rng.NextItem(Owner.Potions)` (its belt slot is Lift's argument);
    /// Push is locked when no deck card can take `Vigorous`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let slots: Vec<usize> = ctx.state().potions_held().map(|(slot, _)| slot).collect();
        let lift = match ctx.rng.next_item(&slots) {
            Some(&slot) => {
                EventOption::with_arg("STONE_OF_ALL_TIME.pages.INITIAL.options.LIFT", "LIFT", slot)
            }
            None => EventOption::locked("STONE_OF_ALL_TIME.pages.INITIAL.options.LIFT_LOCKED"),
        };
        let push = if ctx
            .state()
            .enchantable_cards(EnchantmentKind::Vigorous)?
            .is_empty()
        {
            EventOption::locked("STONE_OF_ALL_TIME.pages.INITIAL.options.PUSH_LOCKED")
        } else {
            EventOption::new("STONE_OF_ALL_TIME.pages.INITIAL.options.PUSH", "PUSH")
        };
        Ok(vec![lift, push])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "LIFT" {
            let s = ctx.state();
            s.discard_potion(action.arg);
            s.gain_max_hp(DRINK_MAX_HP_GAIN);
        } else {
            ctx.state().damage(PUSH_HP_LOSS);
            let kind = EnchantmentKind::Vigorous;
            let options = ctx.state().enchantable_cards(kind)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: PUSH_VIGOROUS_AMOUNT,
            };
            ctx.select(purpose, options, ENCHANTS)?;
        }
        // After the selection in the game; nothing else draws on the event `Rng` in between.
        ctx.rng.next_int(UNUSED_ROLL);
        Ok(Page::finished("STONE_OF_ALL_TIME", action.tag))
    }
}
