//! `ToricToughness` (`Models.Cards/ToricToughness.cs`): event skill, cost 2; gain 5 block (7
//! upgraded), then apply a new `ToricToughnessPower` instance of 2 turns that regains the block
//! actually gained.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::{PowerData, PowerKind, toric_toughness};

pub struct ToricToughness;

#[sts2_macros::hooks]
impl Hooks for ToricToughness {
    type Me = CardId;
}

impl CardModel for ToricToughness {
    fn name(&self) -> &'static str {
        "Toric Toughness"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Named("Turns"), 2),
            CardVar::new(Var::Block, 5).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        let block = eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let power = eng.apply_power_instance(
            player,
            PowerKind::ToricToughness,
            eng.s.card(play.card).var(Var::Named("Turns")),
            Some(player),
        )?;
        // `?.SetBlock(blockAmount)` on the returned instance.
        if let Some(me) = power {
            eng.s.set_power_data(
                me.id,
                PowerData::ToricToughness(toric_toughness::Data { block }),
            );
        }
        Ok(())
    }
}
