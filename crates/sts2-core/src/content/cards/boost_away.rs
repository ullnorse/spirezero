//! `BoostAway` (`Models.Cards/BoostAway.cs`): cost 0, 6 block (9 upgraded), then a `Dazed` into the discard pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, ValueProps};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct BoostAway;

#[sts2_macros::hooks]
impl Hooks for BoostAway {
    type Me = CardId;
}

impl CardModel for BoostAway {
    fn name(&self) -> &'static str {
        "Boost Away"
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
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let card = eng.create_card(CardKind::Dazed);
        eng.add_generated_card_to_combat(card, PileType::Discard, Some(player))?;
        Ok(())
    }
}
