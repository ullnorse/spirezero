//! `ForegoneConclusionPower` (`Models.Powers/ForegoneConclusionPower.cs`): a counter buff; before
//! its owner's hand draw it shuffles if the draw pile is empty, moves `Amount` cards of the owner's
//! choice from the draw pile to the hand, then removes itself.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType, Prompt, PromptText};
use crate::content::powers::{PowerModel, PowerType};

pub struct ForegoneConclusion;

impl PowerModel for ForegoneConclusion {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Foregone Conclusion"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ForegoneConclusion {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        eng.shuffle_if_necessary()?;
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(prompt, Amount)).
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        let options = eng.s.player.draw.to_vec();
        let chosen = eng.choose_cards(
            Prompt::power(eng.s, me, PromptText::Own),
            options,
            amount,
            amount,
        )?;
        eng.add_cards_to_pile(&chosen, PileType::Hand, Position::Bottom)?;
        eng.remove_power(me.owner, me.id)
    }
}
