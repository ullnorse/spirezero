//! `TheCourier` (`Models.Relics/TheCourier.cs`): merchant prices are 20% lower
//! (`price × (1m - 20m / 100m)`) and a bought merchant slot is restocked. It is not sold in shops.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;

/// `1m - DynamicVar("Discount", 20m) / 100m`.
const FACTOR: Decimal = Decimal::new(8, 1);

pub struct TheCourier;

impl RelicModel for TheCourier {
    fn name(&self) -> &'static str {
        "The Courier"
    }

    fn modify_merchant_price(&self, _s: &RunState, _me: usize, price: Decimal) -> Decimal {
        price * FACTOR
    }

    fn should_refill_merchant_entry(&self, _s: &RunState, _me: usize) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for TheCourier {
    type Me = RelicId;
}
