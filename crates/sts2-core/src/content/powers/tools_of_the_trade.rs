//! `ToolsOfTheTradePower` (`Models.Powers/ToolsOfTheTradePower.cs`): a buff (`Counter`); the
//! owner's hand draw is `Amount` larger, then it discards `Amount` cards at its turn start.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow, Prompt, PromptText};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct ToolsOfTheTrade;

impl PowerModel for ToolsOfTheTrade {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Tools of the Trade"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ToolsOfTheTrade {
    type Me = PowerRef;

    fn modify_hand_draw(&self, s: &CombatState, me: PowerRef, count: Decimal) -> Decimal {
        // The only player is the owner.
        count + Decimal::from(s.power(me.owner, me.id).amount)
    }

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // FromHandForDiscard(prefs: Amount): exactly `Amount` cards.
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).expect("positive");
        let options = eng.s.player.hand.to_vec();
        let cards = eng.choose_cards(
            Prompt::power(eng.s, me, PromptText::Discard),
            options,
            amount,
            amount,
        )?;
        if cards.is_empty() {
            return Ok(());
        }
        eng.discard(&cards)
    }
}
