//! `EternalFeather` (`Models.Relics/EternalFeather.cs`): entering a rest site heals 3 per 5 deck
//! cards.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RoomType, RunState};

/// `CardsVar(5)`.
const CARDS: usize = 5;
/// `HealVar(3m)`.
const HEAL: i32 = 3;

pub struct EternalFeather;

impl RelicModel for EternalFeather {
    fn name(&self) -> &'static str {
        "Eternal Feather"
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, _me: usize, room: RoomType) {
        if room == RoomType::RestSite {
            let sets = i32::try_from(s.deck.len() / CARDS).expect("a deck size fits i32");
            s.heal(Decimal::from(HEAL * sets));
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for EternalFeather {
    type Me = RelicId;
}
