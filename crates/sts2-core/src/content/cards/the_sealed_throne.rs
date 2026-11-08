//! `TheSealedThrone` (`Models.Cards/TheSealedThrone.cs`): Regent ancient power, cost 1 (0 upgraded) and 3 stars; gain 1 The Sealed Throne: a star before each card played.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct TheSealedThrone;

#[sts2_macros::hooks]
impl Hooks for TheSealedThrone {
    type Me = CardId;
}

impl CardModel for TheSealedThrone {
    fn name(&self) -> &'static str {
        "The Sealed Throne"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
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
            PowerKind::TheSealedThrone,
            Decimal::ONE,
            Some(player),
        )
    }
}
