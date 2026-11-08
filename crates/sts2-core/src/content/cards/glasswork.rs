//! `Glasswork` (`Models.Cards/Glasswork.cs`): cost 1; 5 block (8 upgraded), then channel 1 Glass.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Glasswork;

#[sts2_macros::hooks]
impl Hooks for Glasswork {
    type Me = CardId;
}

impl CardModel for Glasswork {
    fn name(&self) -> &'static str {
        "Glasswork"
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
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 5).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).int_var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, Decimal::from(block), ValueProps::MOVE, Some(play))?;
        eng.channel(OrbKind::Glass)
    }
}
