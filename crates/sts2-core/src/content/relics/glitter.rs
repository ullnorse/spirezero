//! `Glitter` (`Models.Relics/Glitter.cs`): Ancient (Nonupeipe); every card of a card reward that
//! `Glam` can enchant is offered as a clone enchanted with `Glam` 1
//! (`TryModifyCardRewardOptionsLate`, no `Rng`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::content::relics::RelicModel;
use crate::run::{CardCreation, RunCard, RunState};

/// `CardCmd.Enchant<Glam>(card, 1m)`.
const GLAM_AMOUNT: i32 = 1;

pub struct Glitter;

impl RelicModel for Glitter {
    fn name(&self) -> &'static str {
        "Glitter"
    }

    fn try_modify_card_reward_options_late(
        &self,
        _s: &mut RunState,
        _me: usize,
        cards: &mut Vec<RunCard>,
        _options: CardCreation,
    ) -> bool {
        for card in cards.iter_mut() {
            // A card the engine does not port cannot be taken; leave it as it is.
            if card.can_enchant(EnchantmentKind::Glam).unwrap_or(false) {
                card.enchantment = Some(Enchantment::new(EnchantmentKind::Glam, GLAM_AMOUNT));
                card.calculated_level = None;
            }
        }
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Glitter {
    type Me = RelicId;
}
