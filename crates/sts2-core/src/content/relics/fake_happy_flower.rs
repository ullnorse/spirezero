//! `FakeHappyFlower` (`Models.Relics/FakeHappyFlower.cs`): `FakeMerchant`'s `HappyFlower`, 1 energy
//! at the start of every 5th turn. The count (`[SavedProperty] TurnsSeen`) carries over between
//! combats. Its fields are `HappyFlower`'s, so it keeps them in [`RelicData::HappyFlower`];
//! `AfterCombatEnd` only resets the relic's `Status` (display).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel, happy_flower};
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `DynamicVar("Turns", 5m)`.
const TURNS: i32 = 5;

pub struct FakeHappyFlower;

impl RelicModel for FakeHappyFlower {
    fn name(&self) -> &'static str {
        "Happy Flower???"
    }

    fn init_data(&self) -> RelicData {
        RelicData::HappyFlower(happy_flower::Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeHappyFlower {
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
        let RelicData::HappyFlower(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a FakeHappyFlower has HappyFlower data");
        };
        data.turns_seen = (data.turns_seen + 1) % TURNS;
        if data.turns_seen == 0 {
            eng.gain_energy(ENERGY)?;
        }
        Ok(())
    }
}
