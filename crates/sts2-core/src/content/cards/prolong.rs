//! `Prolong` (`Models.Cards/Prolong.cs`): cost 0, Exhaust (upgrade removes it); gain next turn
//! the Block the player has now (`BlockNextTurnPower`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Prolong;

#[sts2_macros::hooks]
impl Hooks for Prolong {
    type Me = CardId;
}

impl CardModel for Prolong {
    fn name(&self) -> &'static str {
        "Prolong"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = Decimal::from(eng.s.creature(player).block);
        eng.apply_power(player, PowerKind::BlockNextTurn, block, Some(player))
    }
}
