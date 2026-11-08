//! `WellLaidPlans` (`Models.Cards/WellLaidPlans.cs`): cost 2 (1 less upgraded), a Power: applies `WellLaidPlansPower`
//! (Retain the whole hand.).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct WellLaidPlans;

#[sts2_macros::hooks]
impl Hooks for WellLaidPlans {
    type Me = CardId;
}

impl CardModel for WellLaidPlans {
    fn name(&self) -> &'static str {
        "Well-Laid Plans"
    }

    fn cost(&self) -> i32 {
        2
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
            PowerKind::WellLaidPlans,
            Decimal::ONE,
            Some(player),
        )
    }
}
