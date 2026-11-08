//! `PaelsTears` (`Models.Relics/PaelsTears.cs`): Ancient (Pael); if the owner ends its turn with
//! energy left, 2 energy at the start of its next turn. The flag resets after combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `EnergyVar(2)`.
const ENERGY: Decimal = Decimal::new(2, 0);

pub struct PaelsTears;

/// `PaelsTears`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_hadLeftoverEnergy`.
    pub had_leftover_energy: bool,
}

fn data_mut<'a>(eng: &'a mut Engine<'_>, me: RelicId) -> &'a mut Data {
    let RelicData::PaelsTears(data) = &mut eng.s.relic_mut(me).data else {
        unreachable!("a PaelsTears has PaelsTears data");
    };
    data
}

impl RelicModel for PaelsTears {
    fn name(&self) -> &'static str {
        "Pael's Tears"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PaelsTears(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsTears {
    type Me = RelicId;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&eng.s.player.creature) {
            return Ok(());
        }
        let leftover = eng.s.player.energy > 0;
        data_mut(eng, me).had_leftover_energy = leftover;
        Ok(())
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && data_mut(eng, me).had_leftover_energy {
            eng.gain_energy(ENERGY)?;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng, me).had_leftover_energy = false;
        Ok(())
    }
}
