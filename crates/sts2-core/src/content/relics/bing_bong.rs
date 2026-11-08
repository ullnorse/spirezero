//! `BingBong` (`Models.Relics/BingBong.cs`): whenever a card joins the deck, a clone of it joins
//! the bottom of the deck too. Event rarity (`DollRoom`). In combat no card joins the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{RunCard, RunState};

pub struct BingBong;

/// `BingBong`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_cardsToSkip.Count`: every clone joins it and is never removed, since the clone's own
    /// `AfterCardChangedPiles` (`clonedBy == this`) returns before `CardsToSkip.Remove`.
    pub cards_to_skip: u32,
    /// The clone is joining: its `AfterCardChangedPiles` has `clonedBy == this` and is skipped.
    pub adding_clone: bool,
}

fn data_mut(relic: &mut RelicData) -> &mut Data {
    let RelicData::BingBong(data) = relic else {
        unreachable!("a BingBong has BingBong data");
    };
    data
}

impl RelicModel for BingBong {
    fn name(&self) -> &'static str {
        "Bing Bong"
    }

    fn init_data(&self) -> RelicData {
        RelicData::BingBong(Data::default())
    }

    /// `AfterCardChangedPiles` for a card now in the deck, not cloned by a model:
    /// `RunState.CloneCard` (upgrades and enchantment kept), then `CardPileCmd.Add(clone, Deck,
    /// Bottom, clonedBy: this)`, which runs `ModifyCardBeingAddedToDeck` and the other listeners.
    fn after_card_added_to_deck(&self, s: &mut RunState, me: usize, card: RunCard) {
        let data = data_mut(&mut s.relics[me].data);
        if data.adding_clone {
            return;
        }
        data.adding_clone = true;
        data.cards_to_skip += 1;
        s.add_card(card);
        data_mut(&mut s.relics[me].data).adding_clone = false;
    }
}

#[sts2_macros::hooks]
impl Hooks for BingBong {
    type Me = RelicId;
}
