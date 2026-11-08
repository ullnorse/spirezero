//! `BigGameHunter` (`Models.Modifiers/BigGameHunter.cs`): each act's map is generated again with
//! 2.5 times the elites (rounded half to even; the elites ignore the placement rules), and an
//! elite fight's card reward offers rare cards only.

use crate::run::RunState;
use crate::run::map::{ActMap, MapError};
use crate::run::odds::{CardRarity, CardRarityOddsType};
use crate::run::rewards::{
    CardCreation, CardFilter, CardOdds, CardPool, CreationFlags, possible_cards,
};

/// `ModifyGeneratedMap` for a standard map.
pub(super) fn modify_map(s: &RunState, map: &ActMap, act_index: usize) -> Result<ActMap, MapError> {
    ActMap::generate_big_game_hunter(&s.rng, act_index, s.ascension, map)
}

/// `ModifyCardRewardCreationOptions`.
pub(super) fn modify_options(s: &RunState, mut options: CardCreation) -> CardCreation {
    if options.odds != CardOdds::Encounter(CardRarityOddsType::EliteEncounter)
        || options.flags.has(CreationFlags::NO_CARD_POOL_MODIFICATIONS)
        || options.flags.has(CreationFlags::NO_RARITY_MODIFICATION)
    {
        return options;
    }
    options.odds = CardOdds::Uniform;
    options.filter = CardFilter::Rarity(CardRarity::Rare);
    if possible_cards(s.character, options).is_empty() {
        options.pool = CardPool::Character;
    }
    options
}
