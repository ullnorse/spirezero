//! `PullAggro` (`Models.Cards/PullAggro.cs`): cost 2, Summon 4 (5 upgraded), then 7 Block (9).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct PullAggro;

#[sts2_macros::hooks]
impl Hooks for PullAggro {
    type Me = CardId;
}

impl CardModel for PullAggro {
    fn name(&self) -> &'static str {
        "Pull Aggro"
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
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Summon, 4).upgrade(1),
            CardVar::new(Var::Block, 7).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.summon_osty(eng.s.card(play.card).var(Var::Summon))?;
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
