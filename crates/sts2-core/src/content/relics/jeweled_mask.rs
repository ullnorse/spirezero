//! `JeweledMask` (`Models.Relics/JeweledMask.cs`): Ancient (Vakuu); before the first hand draw, a
//! random Power card of the draw pile (preferring non-Innate ones; `CombatCardSelection`) moves to
//! the hand and is free this turn.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, PileType, RelicId};
use crate::content::cards::{CardKeyword, CardType};
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;

pub struct JeweledMask;

impl RelicModel for JeweledMask {
    fn name(&self) -> &'static str {
        "Jeweled Mask"
    }
}

#[sts2_macros::hooks]
impl Hooks for JeweledMask {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number > 1 {
            return Ok(());
        }
        let mut powers: Vec<CardId> = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() == CardType::Power)
            .collect();
        if powers.is_empty() {
            return Ok(());
        }
        let non_innate: Vec<CardId> = powers
            .iter()
            .copied()
            .filter(|&c| !eng.hooked().has_keyword(c, CardKeyword::Innate))
            .collect();
        if !non_innate.is_empty() {
            powers = non_innate;
        }
        let Some(&card) = eng.s.rng[RunRngType::CombatCardSelection].next_item(&powers) else {
            return Ok(());
        };
        eng.s.set_to_free_this_turn(card);
        eng.add_to_pile(card, PileType::Hand, Position::Bottom)
    }
}
