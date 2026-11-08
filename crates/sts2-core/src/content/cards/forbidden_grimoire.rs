//! `ForbiddenGrimoire` (`Models.Cards/ForbiddenGrimoire.cs`): cost 2 (1 upgraded), Eternal power
//! that adds a card-removal reward after each combat (run layer).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct ForbiddenGrimoire;

#[sts2_macros::hooks]
impl Hooks for ForbiddenGrimoire {
    type Me = CardId;
}

impl CardModel for ForbiddenGrimoire {
    fn name(&self) -> &'static str {
        "Forbidden Grimoire"
    }

    fn cost(&self) -> i32 {
        2
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Eternal)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power(
            player,
            PowerKind::ForbiddenGrimoire,
            Decimal::ONE,
            Some(player),
        )
    }
}
