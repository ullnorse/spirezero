//! `HappyFlower` (`Models.Relics/HappyFlower.cs`): 1 energy at the start of every 3rd turn. The
//! count (`[SavedProperty] TurnsSeen`) carries over between combats.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `DynamicVar("Turns", 3m)`.
const TURNS: i32 = 3;

pub struct HappyFlower;

/// `HappyFlower`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TurnsSeen`.
    pub turns_seen: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::HappyFlower(data) = &mut s.relic_mut(me).data else {
        unreachable!("a HappyFlower has HappyFlower data");
    };
    data
}

impl RelicModel for HappyFlower {
    fn name(&self) -> &'static str {
        "Happy Flower"
    }

    fn init_data(&self) -> RelicData {
        RelicData::HappyFlower(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for HappyFlower {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&eng.s.player.creature) {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.turns_seen = (data.turns_seen + 1) % TURNS;
        if data.turns_seen == 0 {
            eng.gain_energy(ENERGY)?;
        }
        Ok(())
    }
}
