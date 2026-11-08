//! `Relax` (`Models.Cards/Relax.cs`): Ancient skill (`PaelsHorn`), cost 3, Exhaust; 16 block
//! (18 upgraded), then 2 cards (3) and 2 energy (3) next turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Relax;

#[sts2_macros::hooks]
impl Hooks for Relax {
    type Me = CardId;
}

impl CardModel for Relax {
    fn name(&self) -> &'static str {
        "Relax"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 16).upgrade(2),
            CardVar::new(Var::Cards, 2).upgrade(1),
            CardVar::new(Var::Energy, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let (block, cards, energy) = (
            card.var(Var::Block),
            card.var(Var::Cards),
            card.var(Var::Energy),
        );
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::DrawCardsNextTurn,
            cards,
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::EnergyNextTurn,
            energy,
            Some(player),
        )?;
        Ok(())
    }
}
