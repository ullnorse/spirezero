//! `Enlightenment` (`Models.Cards/Enlightenment.cs`): cost 0, Exhaust; every card in the hand costs
//! at most 1 this turn (until played), or this combat when upgraded.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

/// The cost `SetThisTurnOrUntilPlayed` / `SetThisCombat` reduce to (`reduceOnly: true`).
const COST: i32 = 1;

pub struct Enlightenment;

#[sts2_macros::hooks]
impl Hooks for Enlightenment {
    type Me = CardId;
}

impl CardModel for Enlightenment {
    fn name(&self) -> &'static str {
        "Enlightenment"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let upgraded = eng.s.card(play.card).is_upgraded();
        for card in eng.s.player.hand.clone() {
            let mut cost = eng.s.energy_cost_mut(card);
            if upgraded {
                cost.reduce_this_combat(COST);
            } else {
                cost.reduce_this_turn_or_until_played(COST);
            }
        }
        Ok(())
    }
}
