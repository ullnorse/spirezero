//! `FakeMerchantsRug` (`Models.Relics/FakeMerchantsRug.cs`): the reward for beating
//! `FakeMerchantMonster`. It does nothing.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;

pub struct FakeMerchantsRug;

impl RelicModel for FakeMerchantsRug {
    fn name(&self) -> &'static str {
        "The Merchant's Rug???"
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeMerchantsRug {
    type Me = RelicId;
}
