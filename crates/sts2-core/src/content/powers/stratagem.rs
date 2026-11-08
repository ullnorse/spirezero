//! `StratagemPower` (`Models.Powers/StratagemPower.cs`): after the draw pile is shuffled, choose
//! `Amount` cards of the draw pile and put them in the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType, Prompt, PromptText};
use crate::content::powers::{PowerModel, PowerType};

pub struct Stratagem;

impl PowerModel for Stratagem {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Stratagem"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Stratagem {
    type Me = PowerRef;

    fn after_shuffle(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns it.
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount)
            .expect("Stratagem has a positive amount");
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(prompt, Amount)).
        let options = eng.s.player.draw.to_vec();
        for card in eng.choose_cards(
            Prompt::power(eng.s, me, PromptText::Own),
            options,
            amount,
            amount,
        )? {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
