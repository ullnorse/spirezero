//! `DarkstonePeriapt` (`Models.Relics/DarkstonePeriapt.cs`): whenever a Curse joins the deck,
//! gain 6 Max HP (`MaxHpVar(6m)`). In combat no card joins the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunState};

/// `MaxHpVar(6m)`.
const MAX_HP: i32 = 6;

pub struct DarkstonePeriapt;

impl RelicModel for DarkstonePeriapt {
    fn name(&self) -> &'static str {
        "Darkstone Periapt"
    }

    fn after_card_added_to_deck(&self, s: &mut RunState, _me: usize, card: RunCard) {
        if card.card_type() == Some(CardType::Curse) {
            s.gain_max_hp(MAX_HP);
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for DarkstonePeriapt {
    type Me = RelicId;
}
