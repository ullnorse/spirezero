//! `BigMushroom` (`Models.Relics/BigMushroom.cs`): 20 max HP when obtained, and the first turn
//! of every combat draws 2 fewer cards. Event rarity (`HungryForMushrooms`).

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(20m)`.
const MAX_HP: i32 = 20;
/// `CardsVar(2)`.
const CARDS: i32 = 2;

pub struct BigMushroom;

impl RelicModel for BigMushroom {
    fn name(&self) -> &'static str {
        "Big Mushroom"
    }

    /// `CreatureCmd.GainMaxHp`; `Grow` only scales the sprite.
    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_hp(MAX_HP);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for BigMushroom {
    type Me = RelicId;

    /// `player != Owner` never holds (one player).
    fn modify_hand_draw(&self, s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        if s.player.turn_number != 1 {
            return count;
        }
        count - Decimal::from(CARDS)
    }
}
