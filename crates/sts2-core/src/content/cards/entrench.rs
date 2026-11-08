//! `Entrench` (`Models.Cards/Entrench.cs`): Event skill, cost 2 (1 upgraded); gain Block equal
//! to the current Block (unpowered, so Dexterity and Frail do not apply).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Entrench;

#[sts2_macros::hooks]
impl Hooks for Entrench {
    type Me = CardId;
}

impl CardModel for Entrench {
    fn name(&self) -> &'static str {
        "Entrench"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng.s.creature(player).block;
        eng.gain_block(
            player,
            block.into(),
            ValueProps::UNPOWERED.with(ValueProps::MOVE),
            Some(play),
        )?;
        Ok(())
    }
}
