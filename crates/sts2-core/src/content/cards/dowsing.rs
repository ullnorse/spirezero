//! `Dowsing` (`Models.Cards/Dowsing.cs`): quest, unplayable (cost -1), no target. Its
//! `BeforeRoomEntered` acts on the deck card (it becomes `Abundance` after 5 `?` rooms), so the
//! run layer runs it (`run::cards`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

/// Quest card added by `DowsingRod`.
pub struct Dowsing;

#[sts2_macros::hooks]
impl Hooks for Dowsing {
    type Me = CardId;
}

impl CardModel for Dowsing {
    fn name(&self) -> &'static str {
        "Dowsing"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Quest
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Quest
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Rooms"), 5)];
        VARS
    }

    /// The setter of `RoomsEntered`: the var is `5 - RoomsEntered`.
    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match (name, card.data) {
            (Var::Named("Rooms"), CardData::Counter(seen)) => {
                value - Decimal::from(i32::try_from(seen).expect("counter fits i32"))
            }
            _ => value,
        }
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
