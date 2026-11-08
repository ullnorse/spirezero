//! `DevourLife` (`Models.Cards/DevourLife.cs`): cost 1 power; applies 1 Devour Life (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct DevourLife;

#[sts2_macros::hooks]
impl Hooks for DevourLife {
    type Me = CardId;
}

impl CardModel for DevourLife {
    fn name(&self) -> &'static str {
        "DevourLife"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::DevourLife), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::DevourLife));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::DevourLife, amount, Some(player))
    }
}
