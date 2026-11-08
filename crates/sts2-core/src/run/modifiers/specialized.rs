//! `Specialized` (`Models.Modifiers/Specialized.cs`): Neow's option rolls one random card of the
//! character's pool (uniform odds, no rarity modification) and adds 5 copies of it to the deck;
//! the rolled card itself is not added.

use crate::run::RunState;
use crate::run::rewards::{CardCreation, CardFilter, CardPool, CreationFlags, create_card_reward};

/// The copies.
const COPIES: usize = 5;

/// The option's effect.
pub(super) fn obtain_cards(s: &mut RunState) {
    let options = CardCreation::non_combat_uniform(CardPool::Character, CardFilter::Any)
        .with_flags(CreationFlags::NO_RARITY_MODIFICATION);
    let card = create_card_reward(s, 1, options)[0];
    for _ in 0..COPIES {
        s.add_card(card);
    }
}
