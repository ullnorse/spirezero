//! `LightningRod` (`Models.Cards/LightningRod.cs`): skill, cost 1; 4 block (7 upgraded), then
//! apply `LightningRodPower` (2).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct LightningRod;

#[sts2_macros::hooks]
impl Hooks for LightningRod {
    type Me = CardId;
}

impl CardModel for LightningRod {
    fn name(&self) -> &'static str {
        "Lightning Rod"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
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
            CardVar::new(Var::Block, 4).upgrade(3),
            CardVar::new(Var::Power(PowerKind::LightningRod), 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).int_var(Var::Block);
        let amount = eng
            .s
            .card(play.card)
            .int_var(Var::Power(PowerKind::LightningRod));
        let player = eng.s.player.creature;
        eng.gain_block(player, Decimal::from(block), ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::LightningRod,
            Decimal::from(amount),
            Some(player),
        )
    }
}
