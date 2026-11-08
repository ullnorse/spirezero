//! `GatherLight` (`Models.Cards/GatherLight.cs`): Regent common skill, cost 1; gain 8 block (11
//! upgraded), then 1 star.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct GatherLight;

#[sts2_macros::hooks]
impl Hooks for GatherLight {
    type Me = CardId;
}

impl CardModel for GatherLight {
    fn name(&self) -> &'static str {
        "Gather Light"
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
            CardVar::new(Var::Block, 8).upgrade(3),
            CardVar::new(Var::Stars, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))
    }
}
