//! `ShadowShield` (`Models.Cards/ShadowShield.cs`): cost 2, 11 Block (15 upgraded), then channel 1 Dark.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct ShadowShield;

#[sts2_macros::hooks]
impl Hooks for ShadowShield {
    type Me = CardId;
}

impl CardModel for ShadowShield {
    fn name(&self) -> &'static str {
        "Shadow Shield"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 11).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.channel(OrbKind::Dark)
    }
}
