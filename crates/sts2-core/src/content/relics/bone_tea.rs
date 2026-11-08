//! `BoneTea` (`Models.Relics/BoneTea.cs`): upgrade every card in the hand at the start of the
//! first turn of the next combat (`CombatsLeft`, 1, a `[SavedProperty]`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};

pub struct BoneTea;

/// `BoneTea`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CombatsLeft`.
    pub combats_left: i32,
}

impl RelicModel for BoneTea {
    fn name(&self) -> &'static str {
        "Bone Tea"
    }

    /// `_combatsLeft = 1`.
    fn init_data(&self) -> RelicData {
        RelicData::BoneTea(Data { combats_left: 1 })
    }
}

#[sts2_macros::hooks]
impl Hooks for BoneTea {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::BoneTea(data) = eng.s.relic(me).data else {
            unreachable!("a BoneTea has BoneTea data");
        };
        // `IsUsedUp`; the only player is the owner.
        if data.combats_left <= 0 || eng.s.player.turn_number > 1 {
            return Ok(());
        }
        // CardCmd.Upgrade(Hand.Cards): in hand order.
        for card in eng.s.player.hand.clone() {
            eng.upgrade_card(card);
        }
        let RelicData::BoneTea(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a BoneTea has BoneTea data");
        };
        data.combats_left -= 1;
        Ok(())
    }
}
