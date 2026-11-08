//! `Pantograph` (`Models.Relics/Pantograph.cs`): heals 25 at the start of a boss fight
//! (`BeforeCombatStart`). `AfterRoomEntered` only sets the relic's glow.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RoomType;

/// `HealVar(25m)`.
const HEAL: i32 = 25;

pub struct Pantograph;

impl RelicModel for Pantograph {
    fn name(&self) -> &'static str {
        "Pantograph"
    }
}

#[sts2_macros::hooks]
impl Hooks for Pantograph {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        if !eng.s.creature(owner).is_dead() && eng.s.room_type == RoomType::Boss {
            eng.heal(owner, Decimal::from(HEAL))?;
        }
        Ok(())
    }
}
