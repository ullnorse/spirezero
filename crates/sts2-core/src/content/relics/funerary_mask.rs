//! `FuneraryMask` (`Models.Relics/FuneraryMask.cs`): before the first hand draw, shuffles 3 new
//! Souls into the draw pile (each created and added in turn, at a random position).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, RelicId};
use crate::content::cards::CardKind;
use crate::content::relics::RelicModel;

/// `CardsVar(3)`.
const CARDS: i32 = 3;

pub struct FuneraryMask;

impl RelicModel for FuneraryMask {
    fn name(&self) -> &'static str {
        "Funerary Mask"
    }
}

#[sts2_macros::hooks]
impl Hooks for FuneraryMask {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number == 1 {
            let owner = eng.s.player.creature;
            for _ in 0..CARDS {
                let soul = eng.create_card(CardKind::Soul);
                eng.add_generated_card_to_combat_at(
                    soul,
                    PileType::Draw,
                    Some(owner),
                    Position::Random,
                )?;
            }
        }
        Ok(())
    }
}
