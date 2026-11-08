//! `Glow` (`Models.Cards/Glow.cs`): Regent common skill, cost 1; gain 1 star (2 upgraded), draw 1 card, and draw 1 more card next turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Glow;

#[sts2_macros::hooks]
impl Hooks for Glow {
    type Me = CardId;
}

impl CardModel for Glow {
    fn name(&self) -> &'static str {
        "Glow"
    }

    fn cost(&self) -> i32 {
        1
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
            CardVar::new(Var::Stars, 1).upgrade(1),
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))?;
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.draw(cards, false)?;
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::DrawCardsNextTurn,
            cards,
            Some(player),
        )?;
        Ok(())
    }
}
