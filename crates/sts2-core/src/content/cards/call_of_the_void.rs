//! `CallOfTheVoid` (`Models.Cards/CallOfTheVoid.cs`): cost 1 power (Ethereal is only a hover
//! tip; upgraded it is Innate); before each hand draw, a random Ethereal card of the character
//! pool joins the hand.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct CallOfTheVoid;

#[sts2_macros::hooks]
impl Hooks for CallOfTheVoid {
    type Me = CardId;
}

impl CardModel for CallOfTheVoid {
    fn may_prompt(&self) -> bool {
        // Its power generates pool cards that prompt (Seance, ...).
        true
    }

    fn name(&self) -> &'static str {
        "Call of the Void"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.apply_power(player, PowerKind::CallOfTheVoid, cards, Some(player))
    }
}
