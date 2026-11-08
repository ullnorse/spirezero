//! `RedSkull` (`Models.Relics/RedSkull.cs`): while the owner's HP is at or below 50% of its max
//! HP it has 3 extra Strength. Checked on entering a combat room and whenever its HP changes.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `DynamicVar("HpThreshold", 50m) / 100m`.
const HP_THRESHOLD: Decimal = Decimal::new(5, 1);
/// `PowerVar<StrengthPower>(3m)`.
const STRENGTH: i32 = 3;

pub struct RedSkull;

/// `RedSkull`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_strengthApplied`.
    pub strength_applied: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::RedSkull(data) = &mut s.relic_mut(me).data else {
        unreachable!("a RedSkull has RedSkull data");
    };
    data
}

/// `ModifyStrengthIfNecessary`.
fn modify_strength_if_necessary(eng: &mut Engine<'_>, me: RelicId) -> Flow {
    let player = eng.s.player.creature;
    let creature = eng.s.creature(player);
    // `CurrentHp > MaxHp * (HpThreshold / 100m)`.
    let above = Decimal::from(creature.hp) > Decimal::from(creature.max_hp) * HP_THRESHOLD;
    let applied = data_mut(eng.s, me).strength_applied;
    if above && applied {
        eng.apply_power(
            player,
            PowerKind::Strength,
            Decimal::from(-STRENGTH),
            Some(player),
        )?;
        data_mut(eng.s, me).strength_applied = false;
    } else if !above && !applied {
        eng.apply_power(
            player,
            PowerKind::Strength,
            Decimal::from(STRENGTH),
            Some(player),
        )?;
        data_mut(eng.s, me).strength_applied = true;
    }
    Ok(())
}

impl RelicModel for RedSkull {
    fn name(&self) -> &'static str {
        "Red Skull"
    }

    fn init_data(&self) -> RelicData {
        RelicData::RedSkull(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for RedSkull {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        // `room is CombatRoom`: this hook only runs in combat.
        modify_strength_if_necessary(eng, me)
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).strength_applied = false;
        Ok(())
    }

    fn after_current_hp_changed(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _creature: CreatureId,
        _delta: Decimal,
    ) -> Flow {
        if eng.s.in_progress {
            modify_strength_if_necessary(eng, me)?;
        }
        Ok(())
    }
}
