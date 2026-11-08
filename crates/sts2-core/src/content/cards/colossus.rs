//! `Colossus` (`Models.Cards/Colossus.cs`): cost 1, 4 Block (7 upgraded), then half damage from Vulnerable attackers this turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Colossus;

#[sts2_macros::hooks]
impl Hooks for Colossus {
    type Me = CardId;
}

impl CardModel for Colossus {
    fn name(&self) -> &'static str {
        "Colossus"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 4).upgrade(3),
            CardVar::new(Var::Named("Colossus"), 1),
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
            PowerKind::Colossus,
            eng.s.card(play.card).var(Var::Named("Colossus")),
            Some(player),
        )
    }
}
