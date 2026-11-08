//! `CreativeAiPower` (`Models.Powers/CreativeAiPower.cs`): a buff (`Counter`); before the owner's
//! hand draw, `Amount` random Power cards of the character's pool are generated into the hand
//! (`CardFactory.GetDistinctForCombat` on `CombatCardGeneration`, one call per card).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType};
use crate::content::cards::{CardType, pools};
use crate::content::powers::{PowerModel, PowerType};

pub struct CreativeAi;

impl PowerModel for CreativeAi {
    fn name(&self) -> &'static str {
        "Creative AI"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for CreativeAi {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        // The pool is the same for every iteration.
        let powers: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Power)
            .collect();
        let mut i = 0;
        // `i < Amount` re-reads the amount on every pass.
        while i < eng.s.power(me.owner, me.id).amount {
            // `FirstOrDefault`: null when the pool is empty.
            if let Some(&card) = eng.get_distinct_for_combat(&powers, 1)?.first() {
                eng.add_generated_card_to_combat(card, PileType::Hand, Some(me.owner))?;
            }
            i += 1;
        }
        Ok(())
    }
}
