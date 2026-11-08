//! `Blur` (`Models.Cards/Blur.cs`): cost 1, 5 Block (8 upgraded); Block is not cleared at the start of the next turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Blur;

#[sts2_macros::hooks]
impl Hooks for Blur {
    type Me = CardId;
}

impl CardModel for Blur {
    fn name(&self) -> &'static str {
        "Blur"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 5).upgrade(3),
            CardVar::new(Var::Named("Blur"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Blur,
            eng.s.card(play.card).var(Var::Named("Blur")),
            Some(player),
        )
    }
}
