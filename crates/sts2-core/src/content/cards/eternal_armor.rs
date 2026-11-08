//! `EternalArmor` (`Models.Cards/EternalArmor.cs`): cost 3 power; gain 9 Plating (12 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct EternalArmor;

#[sts2_macros::hooks]
impl Hooks for EternalArmor {
    type Me = CardId;
}

impl CardModel for EternalArmor {
    fn name(&self) -> &'static str {
        "Eternal Armor"
    }

    fn cost(&self) -> i32 {
        3
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Plating), 9).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let plating = eng.s.card(play.card).var(Var::Power(PowerKind::Plating));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Plating, plating, Some(player))
    }
}
