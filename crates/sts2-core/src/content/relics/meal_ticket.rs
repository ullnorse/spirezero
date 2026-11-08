//! `MealTicket` (`Models.Relics/MealTicket.cs`): entering a merchant room heals 15 while alive.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RoomType, RunState};

/// `HealVar(15m)`.
const HEAL: i32 = 15;

pub struct MealTicket;

impl RelicModel for MealTicket {
    /// `IsAllowed` (`MealTicket.cs:17-20`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Meal Ticket"
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, _me: usize, room: RoomType) {
        if room == RoomType::Shop && s.hp > 0 {
            s.heal(Decimal::from(HEAL));
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for MealTicket {
    type Me = RelicId;
}
