//! `EntropyPower` (`Models.Powers/EntropyPower.cs`): at the start of each of the owner's turns,
//! choose `Amount` hand cards and transform each into a random card of its pool.
//!
//! The transformation stops the combat (`Unsupported`) if it picks a card that is not ported.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, Prompt, PromptText};
use crate::content::cards::pools;
use crate::content::powers::{PowerModel, PowerType};

pub struct Entropy;

impl PowerModel for Entropy {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Entropy"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Entropy {
    type Me = PowerRef;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        // CardSelectCmd.FromHand(CardSelectorPrefs(TransformSelectionPrompt, Amount)): exactly
        // `Amount`, or the whole hand when it is smaller.
        let hand = eng.s.player.hand.to_vec();
        let count = amount.min(hand.len());
        for card in eng.choose_cards(
            Prompt::power(eng.s, me, PromptText::Transform),
            hand,
            count,
            count,
        )? {
            pools::transform_to_random(eng, card)?;
        }
        Ok(())
    }
}
