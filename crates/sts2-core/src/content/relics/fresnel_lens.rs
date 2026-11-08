//! `FresnelLens` (`Models.Relics/FresnelLens.cs`): every card that can take `Nimble` comes
//! enchanted with it (`NimbleAmount` 2) in card rewards (`TryModifyCardRewardOptionsLate`), in
//! the shop (`ModifyMerchantCardCreationResults`) and when added to the deck
//! (`TryModifyCardBeingAddedToDeck`). A card the engine does not port is left alone (its
//! keywords and cost are unknown; it cannot be taken either).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::content::relics::RelicModel;
use crate::run::{CardCreation, RunCard, RunState};

/// `DynamicVar("NimbleAmount", 2m)`.
const NIMBLE_AMOUNT: i32 = 2;

pub struct FresnelLens;

/// `Nimble.CanEnchant(card)`.
fn can_enchant(card: RunCard) -> bool {
    card.can_enchant(EnchantmentKind::Nimble).unwrap_or(false)
}

/// `EnchantCard`: the clone, enchanted with `Nimble`.
const fn enchanted(mut card: RunCard) -> RunCard {
    card.enchantment = Some(Enchantment::new(EnchantmentKind::Nimble, NIMBLE_AMOUNT));
    card
}

/// `EnchantValidCards`.
fn enchant_valid_cards(cards: &mut [RunCard]) {
    for card in cards {
        if can_enchant(*card) {
            *card = enchanted(*card);
        }
    }
}

impl RelicModel for FresnelLens {
    fn name(&self) -> &'static str {
        "Fresnel Lens"
    }

    fn try_modify_card_reward_options_late(
        &self,
        _s: &mut RunState,
        _me: usize,
        cards: &mut Vec<RunCard>,
        _options: CardCreation,
    ) -> bool {
        enchant_valid_cards(cards);
        true
    }

    fn modify_merchant_card_creation_results(
        &self,
        _s: &RunState,
        _me: usize,
        cards: &mut [RunCard],
    ) {
        enchant_valid_cards(cards);
    }

    fn try_modify_card_being_added_to_deck(
        &self,
        _s: &RunState,
        _me: usize,
        card: RunCard,
    ) -> Option<RunCard> {
        can_enchant(card).then(|| enchanted(card))
    }
}

#[sts2_macros::hooks]
impl Hooks for FresnelLens {
    type Me = RelicId;
}
