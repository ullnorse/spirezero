//! `HelloWorldPower` (`Models.Powers/HelloWorldPower.cs`): a counter buff; before its owner's hand
//! draw, if it was present at the turn start (`AmountOnTurnStart >= 1`), `AmountOnTurnStart`
//! distinct Common cards of the owner's character pool are created in the hand
//! (`CombatCardGeneration`).

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType};
use crate::content::cards::{CardRarity, pools};
use crate::content::powers::{PowerModel, PowerType};

pub struct HelloWorld;

impl PowerModel for HelloWorld {
    fn name(&self) -> &'static str {
        "Hello World"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for HelloWorld {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns it.
        let amount = eng.s.power(me.owner, me.id).amount_on_turn_start;
        if eng.s.player.creature != me.owner || amount < 1 {
            return Ok(());
        }
        let commons: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.rarity == CardRarity::Common)
            .collect();
        let count = usize::try_from(amount).unwrap_or(0);
        let picked = eng.get_distinct_for_combat(&commons, count)?;
        let player = eng.s.player.creature;
        eng.add_generated_cards_to_combat(&picked, PileType::Hand, Some(player), Position::Bottom)
    }
}
