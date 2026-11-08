//! `WaterloggedScriptorium` (`Models.Events/WaterloggedScriptorium.cs`): 6 max HP, or pay 55 gold
//! to enchant a card with `Steady`, or 99 gold to enchant two.

use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState, SelectPurpose};

/// `MaxHpVar(6m)`.
const MAX_HP: i32 = 6;
/// `GoldVar(55)`, also `_spawnGoldRequirement`.
const QUILL_GOLD: i32 = 55;
/// `GoldVar("PricklySpongeGold", 99)`.
const SPONGE_GOLD: i32 = 99;
/// `CardsVar(2)`: the sponge's picks (the quill picks 1).
const SPONGE_PICKS: usize = 2;
const QUILL_PICKS: usize = 1;

pub struct WaterloggedScriptorium;

impl EventModel for WaterloggedScriptorium {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= QUILL_GOLD
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = ctx.state().gold;
        let quill = if gold >= QUILL_GOLD {
            EventOption::new(
                "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.TENTACLE_QUILL",
                "TENTACLE_QUILL",
            )
        } else {
            EventOption::locked(
                "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.TENTACLE_QUILL_LOCKED",
            )
        };
        let sponge = if gold >= SPONGE_GOLD {
            EventOption::new(
                "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.PRICKLY_SPONGE",
                "PRICKLY_SPONGE",
            )
        } else {
            EventOption::locked(
                "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.PRICKLY_SPONGE_LOCKED",
            )
        };
        Ok(vec![
            EventOption::new(
                "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.BLOODY_INK",
                "BLOODY_INK",
            ),
            quill,
            sponge,
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let (gold, picks) = match action.tag {
            "TENTACLE_QUILL" => (QUILL_GOLD, QUILL_PICKS),
            "PRICKLY_SPONGE" => (SPONGE_GOLD, SPONGE_PICKS),
            _ => {
                // `BloodyInk`.
                ctx.state().gain_max_hp(MAX_HP);
                return Ok(Page::finished("WATERLOGGED_SCRIPTORIUM", "BLOODY_INK"));
            }
        };
        // `LoseGold(Spent)`, then `FromDeckForEnchantment(Steady, 1)` and `CardCmd.Enchant`
        // on each pick.
        ctx.state().lose_gold(gold);
        let options = ctx.state().enchantable_cards(EnchantmentKind::Steady)?;
        let purpose = SelectPurpose::Enchant {
            kind: EnchantmentKind::Steady,
            amount: 1,
        };
        ctx.select(purpose, options, picks)?;
        Ok(Page::finished("WATERLOGGED_SCRIPTORIUM", action.tag))
    }
}
