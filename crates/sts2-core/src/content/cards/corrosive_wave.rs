//! `CorrosiveWave` (`Models.Cards/CorrosiveWave.cs`): cost 1, applies 2 `CorrosiveWave` (3 upgraded) to the player.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct CorrosiveWave;

#[sts2_macros::hooks]
impl Hooks for CorrosiveWave {
    type Me = CardId;
}

impl CardModel for CorrosiveWave {
    fn name(&self) -> &'static str {
        "Corrosive Wave"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("CorrosiveWave"), 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // DynamicVar("CorrosiveWave")(2m), +1.
        let amount = eng.s.card(play.card).var(Var::Named("CorrosiveWave"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::CorrosiveWave,
            amount,
            Some(player),
        )
    }
}
