//! `Subroutine` (`Models.Cards/Subroutine.cs`): power, cost 1; applies `SubroutinePower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Subroutine;

#[sts2_macros::hooks]
impl Hooks for Subroutine {
    type Me = CardId;
}

impl CardModel for Subroutine {
    fn name(&self) -> &'static str {
        "Subroutine"
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
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = Decimal::ONE;
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Subroutine,
            amount,
            Some(player),
        )
    }
}
