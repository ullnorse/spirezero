//! `BiasedCognition` (`Models.Cards/BiasedCognition.cs`): cost 1 Ancient Power; 5 Focus (6
//! upgraded), then `BiasedCognitionPower` (1), which takes 1 Focus away at the start of each turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct BiasedCognition;

#[sts2_macros::hooks]
impl Hooks for BiasedCognition {
    type Me = CardId;
}

impl CardModel for BiasedCognition {
    fn name(&self) -> &'static str {
        "Biased Cognition"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Focus), 5).upgrade(1),
            CardVar::new(Var::Power(PowerKind::BiasedCognition), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let focus = eng.s.card(play.card).int_var(Var::Power(PowerKind::Focus));
        let decay = eng
            .s
            .card(play.card)
            .int_var(Var::Power(PowerKind::BiasedCognition));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Focus,
            Decimal::from(focus),
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::BiasedCognition,
            Decimal::from(decay),
            Some(player),
        )
    }
}
