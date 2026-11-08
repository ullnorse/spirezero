//! `ReaperForm` (`Models.Cards/ReaperForm.cs`): cost 3 power; applies 1 Reaper Form. The upgrade
//! adds Retain.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct ReaperForm;

#[sts2_macros::hooks]
impl Hooks for ReaperForm {
    type Me = CardId;
}

impl CardModel for ReaperForm {
    fn name(&self) -> &'static str {
        "Reaper Form"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::ReaperForm, Decimal::ONE, Some(player))
    }
}
