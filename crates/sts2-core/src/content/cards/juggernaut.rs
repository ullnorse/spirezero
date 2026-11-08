//! `Juggernaut` (`Models.Cards/Juggernaut.cs`): cost 2, applies `JuggernautPower` (6 damage to a random enemy per Block gained; 8 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Juggernaut;

#[sts2_macros::hooks]
impl Hooks for Juggernaut {
    type Me = CardId;
}

impl CardModel for Juggernaut {
    fn name(&self) -> &'static str {
        "Juggernaut"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Juggernaut), 6).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Juggernaut,
            eng.s.card(play.card).var(Var::Power(PowerKind::Juggernaut)),
            Some(player),
        )
    }
}
