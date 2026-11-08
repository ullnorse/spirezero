//! `Metronome` (`Models.Relics/Metronome.cs`): the 7th orb the owner channels in a combat deals 30
//! Unpowered damage to every hittable enemy. The count (`_orbsChanneled`) is reset on entering a
//! combat room and when the combat ends; the counter display is presentation.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, Orb, RelicId, ValueProps};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `DamageVar(30m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 30;
/// `DynamicVar("OrbCount", 7m)`.
const ORB_COUNT: i32 = 7;

pub struct Metronome;

/// `Metronome`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_orbsChanneled`.
    pub orbs_channeled: i32,
}

impl RelicModel for Metronome {
    fn name(&self) -> &'static str {
        "Metronome"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Metronome(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Metronome {
    type Me = RelicId;

    /// `AfterRoomEntered` of a `CombatRoom` (the engine only dispatches it for fights).
    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        eng.s.relic_mut(me).data = RelicData::Metronome(Data::default());
        Ok(())
    }

    fn after_orb_channeled(&self, eng: &mut Engine<'_>, me: RelicId, _orb: Orb) -> Flow {
        let RelicData::Metronome(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a Metronome has Metronome data");
        };
        data.orbs_channeled += 1;
        if data.orbs_channeled == ORB_COUNT {
            let owner = eng.s.player.creature;
            let enemies = eng.hooked().hittable_enemies();
            eng.damage(
                &enemies,
                Decimal::from(DAMAGE),
                ValueProps::UNPOWERED,
                Some(owner),
                None,
            )?;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        eng.s.relic_mut(me).data = RelicData::Metronome(Data::default());
        Ok(())
    }
}
