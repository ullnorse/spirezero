//! `TyrannyPower` (`Models.Powers/TyrannyPower.cs`): a buff (`Counter`); the owner's hand draw
//! is `Amount` larger, then it exhausts `Amount` hand cards of its choice at its turn start.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow, Prompt, PromptText};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Tyranny;

impl PowerModel for Tyranny {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Tyranny"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Tyranny {
    type Me = PowerRef;

    fn modify_hand_draw(&self, s: &CombatState, me: PowerRef, count: Decimal) -> Decimal {
        // The only player is the owner.
        count + Decimal::from(s.power(me.owner, me.id).amount)
    }

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // FromHand(prefs: ExhaustSelectionPrompt, Amount): exactly `Amount` cards.
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).expect("positive");
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(
            Prompt::power(eng.s, me, PromptText::Exhaust),
            options,
            amount,
            amount,
        )? {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }
}
