//! `NightTerrors` (`Models.Modifiers/NightTerrors.cs`): resting heals to full
//! (`ModifyRestSiteHealAmount` returns the max HP) and costs 5 max HP (never below 1) unless the
//! heal is mimicked (`AfterRestSiteHeal`).

use crate::decimal::Decimal;
use crate::run::RunState;

/// `_maxHpLoss`.
const MAX_HP_LOSS: i32 = 5;

/// `ModifyRestSiteHealAmount`: the creature's max HP.
pub(super) fn heal_amount(s: &RunState) -> Decimal {
    Decimal::from(s.max_hp)
}

/// `AfterRestSiteHeal`: `LoseMaxHp(min(5, MaxHp - 1))`.
pub(super) fn after_heal(s: &mut RunState, is_mimicked: bool) {
    if !is_mimicked {
        s.lose_max_hp(MAX_HP_LOSS.min(s.max_hp - 1));
    }
}
