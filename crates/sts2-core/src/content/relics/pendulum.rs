//! `Pendulum` (`Models.Relics/Pendulum.cs`): draw 1 more card every 3rd turn. The count
//! (`[SavedProperty] TurnsSeen`) carries over between combats.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(1)`.
const CARDS: i32 = 1;
/// `DynamicVar("Turns", 3m)`.
const TURNS: i32 = 3;

pub struct Pendulum;

/// `Pendulum`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TurnsSeen`.
    pub turns_seen: i32,
}

impl RelicModel for Pendulum {
    fn name(&self) -> &'static str {
        "Pendulum"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Pendulum(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Pendulum {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::Pendulum(data) = eng.s.relic_data_mut(me) else {
            unreachable!("a Pendulum has Pendulum data");
        };
        data.turns_seen = (data.turns_seen + 1) % TURNS;
        Ok(())
    }

    fn modify_hand_draw(&self, s: &CombatState, me: RelicId, count: Decimal) -> Decimal {
        let RelicData::Pendulum(data) = *s.relic_data(me) else {
            unreachable!("a Pendulum has Pendulum data");
        };
        if data.turns_seen != 0 {
            return count;
        }
        count + Decimal::from(CARDS)
    }
}
