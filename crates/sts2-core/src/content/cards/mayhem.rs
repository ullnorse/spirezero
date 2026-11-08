//! `Mayhem` (`Models.Cards/Mayhem.cs`): power, cost 2 (1 upgraded); apply `MayhemPower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Mayhem;

#[sts2_macros::hooks]
impl Hooks for Mayhem {
    type Me = CardId;
}

impl CardModel for Mayhem {
    fn may_prompt(&self) -> bool {
        // `MayhemPower` auto-plays cards from the draw pile, which can raise a choice.
        true
    }

    fn name(&self) -> &'static str {
        "Mayhem"
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

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Mayhem, Decimal::ONE, Some(player))
    }
}
