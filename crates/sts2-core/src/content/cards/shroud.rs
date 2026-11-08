//! `Shroud` (`Models.Cards/Shroud.cs`): cost 1 power; applies `ShroudPower` with
//! `BlockVar(3m, Unpowered)` (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Shroud;

#[sts2_macros::hooks]
impl Hooks for Shroud {
    type Me = CardId;
}

impl CardModel for Shroud {
    fn name(&self) -> &'static str {
        "Shroud"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 3)
            .props(ValueProps::UNPOWERED)
            .upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Shroud, amount, Some(player))
    }
}
