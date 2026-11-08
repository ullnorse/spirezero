//! `EscapePlan` (`Models.Cards/EscapePlan.cs`): cost 0, draw 1; if it is a Skill, 3 block (5 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct EscapePlan;

#[sts2_macros::hooks]
impl Hooks for EscapePlan {
    type Me = CardId;
}

impl CardModel for EscapePlan {
    fn name(&self) -> &'static str {
        "Escape Plan"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn gains_block(&self) -> bool {
        true
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let drawn = eng.draw(Decimal::ONE, false)?;
        // `FirstOrDefault()` is a Skill: gain the block.
        if drawn
            .first()
            .is_some_and(|&c| eng.s.card(c).card_type() == CardType::Skill)
        {
            let player = eng.s.player.creature;
            eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        }
        Ok(())
    }
}
