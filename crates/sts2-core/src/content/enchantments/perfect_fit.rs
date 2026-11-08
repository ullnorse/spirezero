//! `PerfectFit` (`Models.Enchantments/PerfectFit.cs`): a reshuffle (not the combat's first
//! shuffle) puts the enchanted card on top of the draw pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState};
use crate::content::enchantments::EnchantmentModel;

pub struct PerfectFit;

impl EnchantmentModel for PerfectFit {
    fn name(&self) -> &'static str {
        "Perfect Fit"
    }
}

#[sts2_macros::hooks]
impl Hooks for PerfectFit {
    type Me = CardId;

    fn modify_shuffle_order(
        &self,
        _s: &CombatState,
        me: CardId,
        cards: &mut Vec<CardId>,
        is_initial: bool,
    ) {
        if let Some(i) = cards.iter().position(|&c| c == me).filter(|_| !is_initial) {
            cards.remove(i);
            cards.insert(0, me);
        }
    }
}
