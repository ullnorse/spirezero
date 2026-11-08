//! `MultiCast` (`Models.Cards/MultiCast.cs`): cost X; evoke the front orb X times (X + 1 upgraded), removing it on the last.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct MultiCast;

#[sts2_macros::hooks]
impl Hooks for MultiCast {
    type Me = CardId;
}

impl CardModel for MultiCast {
    fn name(&self) -> &'static str {
        "Multi-Cast"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // X evokes of the front orb (one more upgraded); only the last dequeues it.
        let upgraded = eng.s.card(play.card).is_upgraded();
        let count = eng.hooked().resolve_energy_x_value(play.card) + i32::from(upgraded);
        for i in 0..count {
            eng.evoke_next(i == count - 1)?;
        }
        Ok(())
    }
}
