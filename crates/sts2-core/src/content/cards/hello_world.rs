//! `HelloWorld` (`Models.Cards/HelloWorld.cs`): Event power, cost 1; each turn, before the hand
//! draw, add a random Common card of the owner's pool to the hand (`HelloWorldPower`). Upgrade
//! adds Innate.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct HelloWorld;

#[sts2_macros::hooks]
impl Hooks for HelloWorld {
    type Me = CardId;
}

impl CardModel for HelloWorld {
    fn name(&self) -> &'static str {
        "Hello World"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
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
            PowerKind::HelloWorld,
            Decimal::ONE,
            Some(player),
        )
    }
}
