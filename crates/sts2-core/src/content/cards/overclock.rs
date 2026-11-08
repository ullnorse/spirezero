//! `Overclock` (`Models.Cards/Overclock.cs`): cost 0, draw 2 (3 upgraded), then a `Burn` into the discard pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Overclock;

#[sts2_macros::hooks]
impl Hooks for Overclock {
    type Me = CardId;
}

impl CardModel for Overclock {
    fn name(&self) -> &'static str {
        "Overclock"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).var(Var::Cards);
        let player = eng.s.player.creature;
        eng.draw(cards, false)?;
        let card = eng.create_card(CardKind::Burn);
        eng.add_generated_card_to_combat(card, PileType::Discard, Some(player))?;
        Ok(())
    }
}
