//! `PrismaticGem` (`Models.Relics/PrismaticGem.cs`): +1 max energy; card rewards draw from every
//! unlocked character's pool (`ModifyCardRewardCreationOptions`), unless the options are
//! `NoCardPoolModifications`, not a card reward, or colorless only.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{CardCreation, CardPool, CreationFlags, RunState};

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct PrismaticGem;

impl RelicModel for PrismaticGem {
    fn name(&self) -> &'static str {
        "Prismatic Gem"
    }

    fn modify_card_reward_creation_options(
        &self,
        _s: &RunState,
        _me: usize,
        mut options: CardCreation,
    ) -> CardCreation {
        if options.flags.has(CreationFlags::NO_CARD_POOL_MODIFICATIONS) || !options.is_card_reward {
            return options;
        }
        // `UnlockState.CharacterCardPools.Union(options.CardPools)`.
        options.pool = match options.pool {
            CardPool::Colorless => return options,
            CardPool::Character
            | CardPool::CharacterAnd(_)
            | CardPool::Of(_)
            | CardPool::AllCharacters => CardPool::AllCharacters,
            CardPool::CharacterAndColorless
            | CardPool::CharacterAndColorlessAnd(_)
            | CardPool::AllCharactersAndColorless => CardPool::AllCharactersAndColorless,
        };
        options
    }
}

#[sts2_macros::hooks]
impl Hooks for PrismaticGem {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }
}
