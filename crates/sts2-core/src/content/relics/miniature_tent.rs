//! `MiniatureTent` (`Models.Relics/MiniatureTent.cs`): after a rest-site option is used the other
//! options stay available (`ShouldDisableRemainingRestSiteOptions` is false).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::RunState;

pub struct MiniatureTent;

impl RelicModel for MiniatureTent {
    fn name(&self) -> &'static str {
        "Miniature Tent"
    }

    fn should_disable_remaining_rest_site_options(&self, _s: &RunState, _me: usize) -> bool {
        false
    }
}

#[sts2_macros::hooks]
impl Hooks for MiniatureTent {
    type Me = RelicId;
}
