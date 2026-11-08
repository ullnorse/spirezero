//! `FishingRod` (`Models.Relics/FishingRod.cs`): every third monster combat, a random upgradable
//! deck card is upgraded (`Rng.Niche`). Its `AfterCombatEnd` acts on the deck, so the run runs it
//! once the fight is over.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::rng::RunRngType;
use crate::run::{RoomType, RunState};

/// `DynamicVar("Combats", 3m)`.
const COMBATS: i32 = 3;

pub struct FishingRod;

/// `FishingRod`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CombatsSeen`.
    pub combats_seen: i32,
}

impl RelicModel for FishingRod {
    fn name(&self) -> &'static str {
        "Fishing Rod"
    }

    fn init_data(&self) -> RelicData {
        RelicData::FishingRod(Data::default())
    }

    fn after_combat_end_on_deck(&self, s: &mut RunState, me: usize, room: RoomType) {
        if room != RoomType::Monster {
            return;
        }
        let RelicData::FishingRod(data) = &mut s.relics[me].data else {
            unreachable!("a FishingRod has FishingRod data");
        };
        data.combats_seen += 1;
        if data.combats_seen % COMBATS == 0 {
            let options = s.upgradable_cards();
            if let Some(&i) = s.rng[RunRngType::Niche].next_item(&options) {
                s.deck[i].upgrade();
            }
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for FishingRod {
    type Me = RelicId;
}
