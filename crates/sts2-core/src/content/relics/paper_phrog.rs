//! `PaperPhrog` (`Models.Relics/PaperPhrog.cs`): the owner's powered attacks make Vulnerable 0.25
//! stronger. It has no hooks; `VulnerablePower` asks it (`ModifyVulnerableMultiplier`) for the
//! dealer's relic.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct PaperPhrog;

impl RelicModel for PaperPhrog {
    fn name(&self) -> &'static str {
        "Paper Phrog"
    }
}

#[sts2_macros::hooks]
impl Hooks for PaperPhrog {
    type Me = RelicId;
}

/// `PaperPhrog.ModifyVulnerableMultiplier(target, amount, props, dealer, cardSource)` for a relic
/// owned by `owner`.
pub fn modify_vulnerable_multiplier(
    owner: CreatureId,
    target: CreatureId,
    amount: Decimal,
    props: ValueProps,
) -> Decimal {
    if target == owner || !props.is_powered_attack() {
        return amount;
    }
    amount + Decimal::new(25, 2)
}
