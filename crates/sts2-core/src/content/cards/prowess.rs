//! `Prowess` (`Models.Cards/Prowess.cs`): power, cost 1; gain 1 Strength and 1 Dexterity (2 each
//! upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Prowess;

#[sts2_macros::hooks]
impl Hooks for Prowess {
    type Me = CardId;
}

impl CardModel for Prowess {
    fn name(&self) -> &'static str {
        "Prowess"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Strength), 1).upgrade(1),
            CardVar::new(Var::Power(PowerKind::Dexterity), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let strength = eng.s.card(play.card).var(Var::Power(PowerKind::Strength));
        let dexterity = eng.s.card(play.card).var(Var::Power(PowerKind::Dexterity));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Strength, strength, Some(player))?;
        eng.apply_power(player, PowerKind::Dexterity, dexterity, Some(player))
    }
}
