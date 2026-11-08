//! `WongoCustomerAppreciationBadge` (`Models.Relics/WongoCustomerAppreciationBadge.cs`): no
//! effect. Event rarity (`WelcomeToWongos`, at 2000 Wongo points).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;

pub struct WongoCustomerAppreciationBadge;

impl RelicModel for WongoCustomerAppreciationBadge {
    fn name(&self) -> &'static str {
        "Wongo Customer Appreciation Badge"
    }
}

#[sts2_macros::hooks]
impl Hooks for WongoCustomerAppreciationBadge {
    type Me = RelicId;
}
