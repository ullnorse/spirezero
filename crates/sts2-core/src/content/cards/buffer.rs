//! `Buffer` (`Models.Cards/Buffer.cs`): Defect rare power, cost 2; gain 1 `BufferPower` (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Buffer;

#[sts2_macros::hooks]
impl Hooks for Buffer {
    type Me = CardId;
}

impl CardModel for Buffer {
    fn name(&self) -> &'static str {
        "Buffer"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Buffer), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Buffer,
            eng.s.card(play.card).var(Var::Power(PowerKind::Buffer)),
            Some(player),
        )
    }
}
