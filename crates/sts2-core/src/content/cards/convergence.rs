//! `Convergence` (`Models.Cards/Convergence.cs`): Regent uncommon skill, cost 1; Retain Hand, 1 energy next turn and 1 star next turn (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Convergence;

#[sts2_macros::hooks]
impl Hooks for Convergence {
    type Me = CardId;
}

impl CardModel for Convergence {
    fn name(&self) -> &'static str {
        "Convergence"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Energy, 1),
            CardVar::new(Var::Stars, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::RetainHand,
            Decimal::ONE,
            Some(player),
        )?;
        let energy = eng.s.card(play.card).var(Var::Energy);
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::EnergyNextTurn,
            energy,
            Some(player),
        )?;
        let stars = eng.s.card(play.card).var(Var::Stars);
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::StarNextTurn,
            stars,
            Some(player),
        )
    }
}
