//! `ChargeBattery` (`Models.Cards/ChargeBattery.cs`): cost 1, 7 block (10 upgraded), then `Energy` (1) `EnergyNextTurn`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct ChargeBattery;

#[sts2_macros::hooks]
impl Hooks for ChargeBattery {
    type Me = CardId;
}

impl CardModel for ChargeBattery {
    fn name(&self) -> &'static str {
        "Charge Battery"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 7).upgrade(3),
            CardVar::new(Var::Energy, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let energy = eng.s.card(play.card).var(Var::Energy);
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::EnergyNextTurn,
            energy,
            Some(player),
        )
    }
}
