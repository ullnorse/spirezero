//! `FrozenEgg` (`Models.Relics/FrozenEgg.cs`): Power cards
//! come upgraded in card rewards (`TryModifyCardRewardOptionsLate`, unless `NoHookUpgrades`), in the shop
//! (`ModifyMerchantCardCreationResults`) and when added to the deck
//! (`TryModifyCardBeingAddedToDeck`). `IsAllowed`: before the Act 3 treasure.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::cards::CardType;
use crate::content::relics::{RelicModel, egg_relic_helper};
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{CardCreation, CreationFlags, RunCard, RunState};

pub struct FrozenEgg;

impl RelicModel for FrozenEgg {
    /// `IsAllowed` (`FrozenEgg.cs:16-19`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Frozen Egg"
    }

    fn try_modify_card_reward_options_late(
        &self,
        _s: &mut RunState,
        _me: usize,
        cards: &mut Vec<RunCard>,
        options: CardCreation,
    ) -> bool {
        if options.flags.has(CreationFlags::NO_HOOK_UPGRADES) {
            return false;
        }
        egg_relic_helper::upgrade_valid_cards(cards, CardType::Power);
        true
    }

    fn modify_merchant_card_creation_results(
        &self,
        _s: &RunState,
        _me: usize,
        cards: &mut [RunCard],
    ) {
        egg_relic_helper::upgrade_valid_cards(cards, CardType::Power);
    }

    fn try_modify_card_being_added_to_deck(
        &self,
        _s: &RunState,
        _me: usize,
        card: RunCard,
    ) -> Option<RunCard> {
        egg_relic_helper::upgraded_clone(card, CardType::Power)
    }
}

#[sts2_macros::hooks]
impl Hooks for FrozenEgg {
    type Me = RelicId;
}
