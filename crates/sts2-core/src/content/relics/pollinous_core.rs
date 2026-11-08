//! `PollinousCore` (`Models.Relics/PollinousCore.cs`): draw 2 more cards every 4th turn. The count
//! (`[SavedProperty] TurnsSeen`) carries over between combats.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(2)`.
const CARDS: i32 = 2;
/// `DynamicVar("Turns", 4m)`.
const TURNS: i32 = 4;

pub struct PollinousCore;

/// `PollinousCore`'s fields (`_isActivating` is the flash animation only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TurnsSeen`.
    pub turns_seen: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::PollinousCore(data) = &mut s.relic_mut(me).data else {
        unreachable!("a PollinousCore has PollinousCore data");
    };
    data
}

impl RelicModel for PollinousCore {
    fn name(&self) -> &'static str {
        "Pollinous Core"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PollinousCore(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for PollinousCore {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).turns_seen += 1;
        Ok(())
    }

    fn modify_hand_draw(&self, s: &CombatState, me: RelicId, count: Decimal) -> Decimal {
        let RelicData::PollinousCore(data) = s.relic(me).data else {
            unreachable!("a PollinousCore has PollinousCore data");
        };
        if data.turns_seen < TURNS {
            return count;
        }
        count + Decimal::from(CARDS)
    }

    /// Runs only when this relic changed the hand draw.
    fn after_modifying_hand_draw(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).turns_seen = 0;
        Ok(())
    }
}
