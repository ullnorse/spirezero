//! `BloodWall` (`Models.Cards/BloodWall.cs`): cost 2, loses 2 HP (unblockable, unpowered), then gains 16 block (20 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct BloodWall;

#[sts2_macros::hooks]
impl Hooks for BloodWall {
    type Me = CardId;
}

impl CardModel for BloodWall {
    fn name(&self) -> &'static str {
        "Blood Wall"
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
            CardVar::new(Var::HpLoss, 2),
            CardVar::new(Var::Block, 16).upgrade(4),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.damage(
            &[player],
            eng.s.card(play.card).var(Var::HpLoss),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(player),
            Some(play.card),
        )?;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
