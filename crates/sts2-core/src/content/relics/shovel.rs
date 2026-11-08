//! `Shovel` (`Models.Relics/Shovel.cs`): adds the Dig option to every rest site; Dig obtains the
//! next relic from the front of the bag (`DigRestSiteOption`). `IsAllowed`: before the Act 3 treasure.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RestOption, RestOptionKind, RunState};

pub struct Shovel;

impl RelicModel for Shovel {
    /// `IsAllowed` (`Shovel.cs:13-16`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Shovel"
    }

    fn try_modify_rest_site_options(
        &self,
        _s: &RunState,
        _me: usize,
        options: &mut Vec<RestOption>,
    ) -> bool {
        options.push(RestOption {
            kind: RestOptionKind::Dig,
            enabled: true,
        });
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Shovel {
    type Me = RelicId;
}
