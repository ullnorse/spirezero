//! `Glacier` (`Models.Cards/Glacier.cs`): cost 2, 6 Block (9 upgraded), then channel 2 Frost.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Glacier;

#[sts2_macros::hooks]
impl Hooks for Glacier {
    type Me = CardId;
}

impl CardModel for Glacier {
    fn name(&self) -> &'static str {
        "Glacier"
    }

    fn cost(&self) -> i32 {
        2
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

    fn gains_block(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        for _ in 0..2 {
            eng.channel(OrbKind::Frost)?;
        }
        Ok(())
    }
}
