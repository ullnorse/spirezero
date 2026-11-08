//! `GraveWarden` (`Models.Cards/GraveWarden.cs`): cost 1, 8 Block (11 upgraded), then shuffles
//! 1 new Soul into the draw pile at a random position.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, ValueProps};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct GraveWarden;

#[sts2_macros::hooks]
impl Hooks for GraveWarden {
    type Me = CardId;
}

impl CardModel for GraveWarden {
    fn name(&self) -> &'static str {
        "Grave Warden"
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
            CardVar::new(Var::Block, 8).upgrade(3),
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let souls: Vec<CardId> = (0..eng.s.card(play.card).int_var(Var::Cards))
            .map(|_| eng.create_card(CardKind::Soul))
            .collect();
        eng.add_generated_cards_to_combat(&souls, PileType::Draw, Some(player), Position::Random)
    }
}
