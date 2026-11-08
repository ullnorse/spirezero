//! `ToolsOfTheTrade` (`Models.Cards/ToolsOfTheTrade.cs`): cost 1 (1 less upgraded), a Power: applies `ToolsOfTheTradePower`
//! (Draw 1 more card each turn, then discard 1.).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct ToolsOfTheTrade;

#[sts2_macros::hooks]
impl Hooks for ToolsOfTheTrade {
    type Me = CardId;
}

impl CardModel for ToolsOfTheTrade {
    fn name(&self) -> &'static str {
        "Tools of the Trade"
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
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::ToolsOfTheTrade,
            Decimal::ONE,
            Some(player),
        )
    }
}
