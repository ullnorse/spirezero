//! `TrashToTreasure` (`Models.Cards/TrashToTreasure.cs`): power, cost 1 (0 upgraded); apply
//! `TrashToTreasurePower` (1, a literal in the C#, not a var).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct TrashToTreasure;

#[sts2_macros::hooks]
impl Hooks for TrashToTreasure {
    type Me = CardId;
}

impl CardModel for TrashToTreasure {
    fn name(&self) -> &'static str {
        "Trash to Treasure"
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
            PowerKind::TrashToTreasure,
            Decimal::ONE,
            Some(player),
        )
    }
}
