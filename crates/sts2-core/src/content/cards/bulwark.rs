//! `Bulwark` (`Models.Cards/Bulwark.cs`): Regent uncommon skill, cost 2; gain 12 block (15
//! upgraded), then Forge 10 (13 upgraded).

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Bulwark;

#[sts2_macros::hooks]
impl Hooks for Bulwark {
    type Me = CardId;
}

impl CardModel for Bulwark {
    fn name(&self) -> &'static str {
        "Bulwark"
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
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 12).upgrade(3),
            CardVar::new(Var::Forge, 10).upgrade(3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let forge = eng.s.card(play.card).var(Var::Forge);
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}
