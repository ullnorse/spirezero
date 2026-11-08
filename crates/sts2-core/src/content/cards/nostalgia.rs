//! `Nostalgia` (`Models.Cards/Nostalgia.cs`): power, cost 1 (0 upgraded); apply 1
//! `NostalgiaPower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Nostalgia;

#[sts2_macros::hooks]
impl Hooks for Nostalgia {
    type Me = CardId;
}

impl CardModel for Nostalgia {
    fn name(&self) -> &'static str {
        "Nostalgia"
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

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Nostalgia, Decimal::ONE, Some(player))
    }
}
