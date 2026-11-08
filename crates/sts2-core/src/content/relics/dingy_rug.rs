//! `DingyRug` (`Models.Relics/DingyRug.cs`): card rewards also draw from the Colorless pool
//! (`ModifyCardRewardCreationOptions` unions the pools), unless the options forbid pool
//! modifications.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{CardCreation, CreationFlags, RunState};

pub struct DingyRug;

impl RelicModel for DingyRug {
    fn name(&self) -> &'static str {
        "Dingy Rug"
    }

    fn modify_card_reward_creation_options(
        &self,
        _s: &RunState,
        _me: usize,
        options: CardCreation,
    ) -> CardCreation {
        if options.flags.has(CreationFlags::NO_CARD_POOL_MODIFICATIONS) || !options.is_card_reward {
            return options;
        }
        CardCreation {
            pool: options.pool.union_colorless(),
            ..options
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for DingyRug {
    type Me = RelicId;
}
