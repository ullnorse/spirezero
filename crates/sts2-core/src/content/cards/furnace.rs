//! `Furnace` (`Models.Cards/Furnace.cs`): Regent uncommon power, cost 1; apply `FurnacePower`
//! with the Forge amount, 5 (7 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Furnace;

#[sts2_macros::hooks]
impl Hooks for Furnace {
    type Me = CardId;
}

impl CardModel for Furnace {
    fn name(&self) -> &'static str {
        "Furnace"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Forge, 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let forge = eng.s.card(play.card).var(Var::Forge);
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Furnace,
            forge,
            Some(player),
        )
    }
}
