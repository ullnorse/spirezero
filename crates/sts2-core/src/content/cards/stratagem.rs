//! `Stratagem` (`Models.Cards/Stratagem.cs`): power, cost 1 (0 upgraded); apply `StratagemPower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Stratagem;

#[sts2_macros::hooks]
impl Hooks for Stratagem {
    type Me = CardId;
}

impl CardModel for Stratagem {
    fn may_prompt(&self) -> bool {
        // The power picks cards from the draw pile after a shuffle.
        true
    }

    fn name(&self) -> &'static str {
        "Stratagem"
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

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Stratagem, Decimal::ONE, Some(player))
    }
}
