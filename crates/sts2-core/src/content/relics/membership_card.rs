//! `MembershipCard` (`Models.Relics/MembershipCard.cs`): merchant prices are halved
//! (`price × (50m / 100m)`, truncated by `MerchantEntry.Cost`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;

/// `DynamicVar("Discount", 50m) / 100m`.
const FACTOR: Decimal = Decimal::new(5, 1);

pub struct MembershipCard;

impl RelicModel for MembershipCard {
    fn name(&self) -> &'static str {
        "Membership Card"
    }

    fn modify_merchant_price(&self, _s: &RunState, _me: usize, price: Decimal) -> Decimal {
        price * FACTOR
    }
}

#[sts2_macros::hooks]
impl Hooks for MembershipCard {
    type Me = RelicId;
}
