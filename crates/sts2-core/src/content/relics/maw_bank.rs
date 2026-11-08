//! `MawBank` (`Models.Relics/MawBank.cs`): gain 12 gold on entering each room of the map (the
//! run's base room: not an event's own fight), until something is bought for gold
//! (`HasItemBeenBought`, which also makes it used up).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::run::map::MapPointType;
use crate::run::{RoomType, RunState};

/// `GoldVar(12)`.
const GOLD: i32 = 12;

pub struct MawBank;

/// `MawBank`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `HasItemBeenBought`.
    pub has_item_been_bought: bool,
}

fn data_mut(s: &mut RunState, me: usize) -> &mut Data {
    match &mut s.relics[me].data {
        RelicData::MawBank(data) => data,
        _ => unreachable!("a MawBank has MawBank data"),
    }
}

/// `AfterRoomEntered` of the base room (`Owner.RunState.BaseRoom == room`).
fn gain_unless_bought(s: &mut RunState, me: usize) {
    if !data_mut(s, me).has_item_been_bought {
        // Flash is visual only.
        s.gain_gold(Decimal::from(GOLD));
    }
}

impl RelicModel for MawBank {
    fn name(&self) -> &'static str {
        "Maw Bank"
    }

    fn init_data(&self) -> RelicData {
        RelicData::MawBank(Data::default())
    }

    /// `IsUsedUp => HasItemBeenBought`.
    fn is_used_up(&self, data: &RelicData) -> bool {
        matches!(data, RelicData::MawBank(d) if d.has_item_been_bought)
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, me: usize, _room: RoomType) {
        gain_unless_bought(s, me);
    }

    fn after_map_combat_room_entered(&self, s: &mut RunState, me: usize, _point: MapPointType) {
        gain_unless_bought(s, me);
    }

    fn after_item_purchased(&self, s: &mut RunState, me: usize, gold_spent: i32) {
        // `player != Owner`: the owner is the only player.
        if gold_spent > 0 {
            data_mut(s, me).has_item_been_bought = true;
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for MawBank {
    type Me = RelicId;
}
