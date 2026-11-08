//! `SpiritOfAsh` (`Models.Cards/SpiritOfAsh.cs`): cost 1 power; applies `SpiritOfAshPower` 4
//! (5 upgraded). The Ethereal in the inventory is a hover tip, not a canonical keyword.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SpiritOfAsh;

#[sts2_macros::hooks]
impl Hooks for SpiritOfAsh {
    type Me = CardId;
}

impl CardModel for SpiritOfAsh {
    fn name(&self) -> &'static str {
        "Spirit of Ash"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("BlockOnExhaust"), 4).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Named("BlockOnExhaust"));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::SpiritOfAsh, amount, Some(player))
    }
}
