//! `Reboot` (`Models.Cards/Reboot.cs`): Defect rare skill, cost 0, Exhaust; put the whole hand
//! on the bottom of the draw pile, shuffle, then draw 4 cards (6 upgraded).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Reboot;

#[sts2_macros::hooks]
impl Hooks for Reboot {
    type Me = CardId;
}

impl CardModel for Reboot {
    fn name(&self) -> &'static str {
        "Reboot"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let hand = eng.s.player.hand.to_vec();
        for card in hand {
            eng.add_to_pile(card, PileType::Draw, Position::Bottom)?;
        }
        eng.shuffle()?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
