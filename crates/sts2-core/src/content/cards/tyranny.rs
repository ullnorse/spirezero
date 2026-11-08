//! `Tyranny` (`Models.Cards/Tyranny.cs`): Regent rare power, cost 1; apply 1 `TyrannyPower`.
//! Upgrade: Innate.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Tyranny;

#[sts2_macros::hooks]
impl Hooks for Tyranny {
    type Me = CardId;
}

impl CardModel for Tyranny {
    fn name(&self) -> &'static str {
        "Tyranny"
    }

    fn cost(&self) -> i32 {
        1
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
        card.keywords.insert(CardKeyword::Innate);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Tyranny,
            Decimal::ONE,
            Some(player),
        )
    }
}
