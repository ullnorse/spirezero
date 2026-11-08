//! `CloakAndDagger` (`Models.Cards/CloakAndDagger.cs`): cost 1, Skill, Common; 6 Block, then
//! creates 1 Shiv in the hand (2 upgraded; the block does not change).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::shiv;
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct CloakAndDagger;

#[sts2_macros::hooks]
impl Hooks for CloakAndDagger {
    type Me = CardId;
}

impl CardModel for CloakAndDagger {
    fn name(&self) -> &'static str {
        "Cloak and Dagger"
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
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 6),
            CardVar::new(Var::Cards, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        for _ in 0..cards {
            shiv::create_in_hand(eng, 1)?;
        }
        Ok(())
    }
}
