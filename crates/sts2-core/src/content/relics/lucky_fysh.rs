//! `LuckyFysh` (`Models.Relics/LuckyFysh.cs`): whenever a card joins the deck, 15 gold.
//!
//! Its `IsAllowed` is `IsBeforeAct3TreasureChest`; `IsAllowedInShops` is false
//! (`run/shop.rs`). In combat no card joins the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RunCard, RunState};

/// `GoldVar(15)`.
const GOLD: i32 = 15;

pub struct LuckyFysh;

impl RelicModel for LuckyFysh {
    /// `IsAllowed` (`LuckyFysh.cs:19-22`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Lucky Fysh"
    }

    fn after_card_added_to_deck(&self, s: &mut RunState, _me: usize, _card: RunCard) {
        s.gain_gold(Decimal::from(GOLD));
    }
}

#[sts2_macros::hooks]
impl Hooks for LuckyFysh {
    type Me = RelicId;
}
