//! `RefineBlade` (`Models.Cards/RefineBlade.cs`): Regent common skill, cost 1; Forge 8 (12
//! upgraded), then 1 `EnergyNextTurnPower`.

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct RefineBlade;

#[sts2_macros::hooks]
impl Hooks for RefineBlade {
    type Me = CardId;
}

impl CardModel for RefineBlade {
    fn name(&self) -> &'static str {
        "Refine Blade"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Forge, 8).upgrade(4),
            CardVar::new(Var::Energy, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let forge = eng.s.card(play.card).var(Var::Forge);
        eng.forge(forge, Some(Listener::Card(play.card)))?;
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
