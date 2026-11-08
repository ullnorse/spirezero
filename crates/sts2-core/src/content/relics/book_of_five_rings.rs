//! `BookOfFiveRings` (`Models.Relics/BookOfFiveRings.cs`): heals 20 HP for every 5 cards added to
//! the deck. Only the deck matters, so the run layer's `after_card_added_to_deck` runs it and the
//! combat never hears it. The count (`[SavedProperty] CardsAdded`) is kept for the run.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RunCard, RunState};

/// `CardsVar(5)`.
const CARDS: i32 = 5;
/// `HealVar(20m)`.
const HEAL: i32 = 20;

pub struct BookOfFiveRings;

/// `BookOfFiveRings`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CardsAdded`.
    pub cards_added: i32,
}

impl RelicModel for BookOfFiveRings {
    /// `IsAllowed` (`BookOfFiveRings.cs:72-75`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Book of Five Rings"
    }

    fn init_data(&self) -> RelicData {
        RelicData::BookOfFiveRings(Data::default())
    }

    fn after_card_added_to_deck(&self, s: &mut RunState, me: usize, _card: RunCard) {
        if s.hp <= 0 {
            return;
        }
        let RelicData::BookOfFiveRings(data) = &mut s.relics[me].data else {
            unreachable!("a BookOfFiveRings has BookOfFiveRings data");
        };
        data.cards_added += 1;
        if data.cards_added % CARDS == 0 {
            s.heal(Decimal::from(HEAL));
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for BookOfFiveRings {
    type Me = RelicId;
}
