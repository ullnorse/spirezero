//! `CallOfTheVoidPower` (`Models.Powers/CallOfTheVoidPower.cs`): before its owner's hand draw,
//! `Amount` random character pool cards (not Basic or Ancient), each from its own
//! `GetDistinctForCombat`, are made Ethereal and added to the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType};
use crate::content::cards::{CardKeyword, CardRarity, pools};
use crate::content::powers::{PowerModel, PowerType};

pub struct CallOfTheVoid;

impl PowerModel for CallOfTheVoid {
    fn may_prompt(&self) -> bool {
        // Generated pool cards prompt (Seance, ...).
        true
    }

    fn name(&self) -> &'static str {
        "Call of the Void"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for CallOfTheVoid {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let options: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| !matches!(c.rarity, CardRarity::Basic | CardRarity::Ancient))
            .collect();
        if options.is_empty() {
            return Ok(());
        }
        // The amount is read once, for the array size.
        let amount = eng.s.power(me.owner, me.id).amount;
        let mut cards = Vec::new();
        for _ in 0..amount {
            let card = eng.get_distinct_for_combat(&options, 1)?[0];
            eng.apply_keyword(card, CardKeyword::Ethereal);
            cards.push(card);
        }
        // Flash: visual only.
        eng.add_generated_cards_to_combat(&cards, PileType::Hand, Some(me.owner), Position::Bottom)
    }
}
