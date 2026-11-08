//! `Sidestep` (`Models.Cards/Sidestep.cs`): cost 0, 1 Energy next turn (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Sidestep;

#[sts2_macros::hooks]
impl Hooks for Sidestep {
    type Me = CardId;
}

impl CardModel for Sidestep {
    fn name(&self) -> &'static str {
        "Sidestep"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::EnergyNextTurn,
            eng.s.card(play.card).var(Var::Energy),
            Some(player),
        )
    }
}
