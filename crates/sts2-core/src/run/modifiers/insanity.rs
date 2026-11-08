//! `Insanity` (`Models.Modifiers/Insanity.cs`): the deck starts empty; Neow's option adds 30
//! random cards of the character's pool (each a `CreateForReward` of 1, uniform odds, no rarity
//! modification: repeats are possible), then `PandorasBox` leaves the relic bags.

use crate::run::RunState;
use crate::run::rewards::{CardCreation, CardFilter, CardPool, CreationFlags, create_card_reward};

/// The cards.
const CARDS: usize = 30;

/// The option's effect.
pub(super) fn obtain_cards(s: &mut RunState) {
    let options = CardCreation::non_combat_uniform(CardPool::Character, CardFilter::Any)
        .with_flags(CreationFlags::NO_RARITY_MODIFICATION);
    for _ in 0..CARDS {
        let card = create_card_reward(s, 1, options)[0];
        s.add_card(card);
    }
    super::remove_pandoras_box(s);
}
