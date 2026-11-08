//! `Terraforming` (`Models.Cards/Terraforming.cs`): Regent uncommon skill, cost 1; gain 7 Vigor (10 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Terraforming;

#[sts2_macros::hooks]
impl Hooks for Terraforming {
    type Me = CardId;
}

impl CardModel for Terraforming {
    fn name(&self) -> &'static str {
        "Terraforming"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Vigor), 7).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Vigor,
            eng.s.card(play.card).var(Var::Power(PowerKind::Vigor)),
            Some(player),
        )?;
        Ok(())
    }
}
