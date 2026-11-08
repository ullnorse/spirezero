//! `CharacterCards` (`Models.Modifiers/CharacterCards.cs`): another character's cards join the
//! card rewards (the pool is `CardPools.Union([that pool])`, for options with `IsCardReward` and
//! without `NoCardPoolModifications`) and the merchant's character cards. Every pool is
//! unioned, a colorless-only one too, but no colorless card reward has `IsCardReward` (the
//! events and `LeadPaperweight` build them without it), so the colorless pool is never widened.

use crate::content::characters::Character;
use crate::run::rewards::{CardCreation, CardPool, CreationFlags, ExtraPools};

/// `ModifyCardRewardCreationOptions`.
pub(super) fn modify_options(other: Character, mut options: CardCreation) -> CardCreation {
    if options.flags.has(CreationFlags::NO_CARD_POOL_MODIFICATIONS) || !options.is_card_reward {
        return options;
    }
    options.pool = match options.pool {
        CardPool::Character => CardPool::CharacterAnd(ExtraPools::of(other)),
        CardPool::CharacterAnd(others) => CardPool::CharacterAnd(others.with(other)),
        CardPool::CharacterAndColorless => {
            CardPool::CharacterAndColorlessAnd(ExtraPools::of(other))
        }
        CardPool::CharacterAndColorlessAnd(others) => {
            CardPool::CharacterAndColorlessAnd(others.with(other))
        }
        pool => pool,
    };
    options
}
