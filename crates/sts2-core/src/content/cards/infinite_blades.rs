//! `InfiniteBlades` (`Models.Cards/InfiniteBlades.cs`): cost 1, Power, Uncommon; applies
//! `InfiniteBladesPower`. Upgrading adds Innate.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct InfiniteBlades;

#[sts2_macros::hooks]
impl Hooks for InfiniteBlades {
    type Me = CardId;
}

impl CardModel for InfiniteBlades {
    fn name(&self) -> &'static str {
        "Infinite Blades"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
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
            PowerKind::InfiniteBlades,
            Decimal::ONE,
            Some(player),
        )
    }
}
