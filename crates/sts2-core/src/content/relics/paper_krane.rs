//! `PaperKrane` (`Models.Relics/PaperKrane.cs`): enemies' Weak takes 15% less off their powered
//! attack damage against the owner. It has no hooks; `WeakPower` asks it
//! (`ModifyWeakMultiplier`) for the target's relic.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct PaperKrane;

impl RelicModel for PaperKrane {
    fn name(&self) -> &'static str {
        "Paper Krane"
    }
}

#[sts2_macros::hooks]
impl Hooks for PaperKrane {
    type Me = RelicId;
}

/// `PaperKrane.ModifyWeakMultiplier(target, amount, props, dealer, cardSource)` for a relic
/// owned by `owner`.
pub fn modify_weak_multiplier(
    owner: CreatureId,
    target: CreatureId,
    amount: Decimal,
    props: ValueProps,
) -> Decimal {
    if target != owner || !props.is_powered_attack() {
        return amount;
    }
    amount - Decimal::new(15, 2)
}
