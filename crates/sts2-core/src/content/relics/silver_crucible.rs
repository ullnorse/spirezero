//! `SilverCrucible` (`Models.Relics/SilverCrucible.cs`): the first 3 card rewards come upgraded,
//! and the first treasure room is empty.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{CardCreation, RoomType, RunCard, RunState};

/// `CardsVar(3)`.
const CARDS: i32 = 3;

pub struct SilverCrucible;

/// `SilverCrucible`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TimesUsed`.
    pub times_used: i32,
    /// `TreasureRoomsEntered`.
    pub treasure_rooms_entered: i32,
}

fn data(s: &RunState, me: usize) -> Data {
    match s.relics[me].data {
        RelicData::SilverCrucible(data) => data,
        _ => unreachable!("a SilverCrucible has SilverCrucible data"),
    }
}

fn data_mut(s: &mut RunState, me: usize) -> &mut Data {
    match &mut s.relics[me].data {
        RelicData::SilverCrucible(data) => data,
        _ => unreachable!("a SilverCrucible has SilverCrucible data"),
    }
}

impl RelicModel for SilverCrucible {
    fn name(&self) -> &'static str {
        "Silver Crucible"
    }

    fn init_data(&self) -> RelicData {
        RelicData::SilverCrucible(Data::default())
    }

    fn try_modify_card_reward_options_late(
        &self,
        s: &mut RunState,
        me: usize,
        cards: &mut Vec<RunCard>,
        options: CardCreation,
    ) -> bool {
        if data(s, me).times_used >= CARDS || !options.is_card_reward {
            return false;
        }
        for card in cards.iter_mut().filter(|c| c.is_upgradable()) {
            card.upgrade();
        }
        true
    }

    fn after_modifying_card_reward_options(&self, s: &mut RunState, me: usize) {
        let data = data_mut(s, me);
        if data.times_used < CARDS {
            data.times_used += 1;
        }
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, me: usize, room: RoomType) {
        if room == RoomType::Treasure {
            data_mut(s, me).treasure_rooms_entered += 1;
        }
    }

    fn should_generate_treasure(&self, s: &RunState, me: usize) -> bool {
        data(s, me).treasure_rooms_entered > 1
    }
}

#[sts2_macros::hooks]
impl Hooks for SilverCrucible {
    type Me = RelicId;
}
