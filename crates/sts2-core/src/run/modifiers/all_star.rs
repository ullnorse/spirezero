//! `AllStar` (`Models.Modifiers/AllStar.cs`): Neow's option adds 5 random colorless cards to the
//! deck, each a `CreateForReward` of 1 with uniform odds and no pool or rarity modification.

use crate::run::RunState;
use crate::run::rewards::{CardCreation, CardFilter, CardPool, CreationFlags, create_card_reward};

/// The cards.
const CARDS: usize = 5;

/// The option's effect.
pub(super) fn obtain_cards(s: &mut RunState) {
    let options = CardCreation::non_combat_uniform(CardPool::Colorless, CardFilter::Any)
        .with_flags(CreationFlags::NO_RARITY_MODIFICATION)
        .without_pool_modifications();
    for _ in 0..CARDS {
        let card = create_card_reward(s, 1, options)[0];
        s.add_card(card);
    }
}
