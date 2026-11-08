//! `ParticleWall` (`Models.Cards/ParticleWall.cs`): Regent uncommon skill, cost 0 and 2 stars;
//! gain 9 block (12 upgraded). A play that would discard it returns it to the hand instead.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ResultLocation, ResultPile, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct ParticleWall;

#[sts2_macros::hooks]
impl Hooks for ParticleWall {
    type Me = CardId;
}

impl CardModel for ParticleWall {
    fn name(&self) -> &'static str {
        "Particle Wall"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 9).upgrade(3)];
        VARS
    }

    fn result_location_for_card_play(&self, base: ResultLocation) -> ResultLocation {
        if base.pile == ResultPile::Discard {
            ResultLocation {
                pile: ResultPile::Hand,
                ..base
            }
        } else {
            base
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
