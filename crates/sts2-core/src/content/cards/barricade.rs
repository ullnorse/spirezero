//! `Barricade` (`Models.Cards/Barricade.cs`): cost 3 (2 upgraded), applies `BarricadePower`: your Block is not removed at the start of your turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Barricade;

#[sts2_macros::hooks]
impl Hooks for Barricade {
    type Me = CardId;
}

impl CardModel for Barricade {
    fn name(&self) -> &'static str {
        "Barricade"
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
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Barricade,
            Decimal::ONE,
            Some(player),
        )
    }
}
