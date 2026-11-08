//! `LizardTail` (`Models.Relics/LizardTail.cs`): once, when the owner would die, it lives and
//! heals 50% of its max HP (at least 1). `WasUsed` is `[SavedProperty]`.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::run::RunState;

/// `HealVar(50m)` as a fraction: `HealVar / 100m`.
const HEAL_FRACTION: Decimal = Decimal::new(5, 1);

pub struct LizardTail;

/// `LizardTail`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `WasUsed`.
    pub was_used: bool,
}

impl RelicModel for LizardTail {
    fn name(&self) -> &'static str {
        "Lizard Tail"
    }

    fn init_data(&self) -> RelicData {
        RelicData::LizardTail(Data::default())
    }

    /// `IsUsedUp => _wasUsed`.
    fn is_used_up(&self, data: &RelicData) -> bool {
        matches!(data, RelicData::LizardTail(d) if d.was_used)
    }

    fn should_die_late_out_of_combat(&self, s: &RunState, me: usize) -> bool {
        let RelicData::LizardTail(data) = s.relics[me].data else {
            unreachable!("a LizardTail has LizardTail data");
        };
        data.was_used
    }

    fn after_preventing_death_out_of_combat(&self, s: &mut RunState, me: usize) {
        let RelicData::LizardTail(data) = &mut s.relics[me].data else {
            unreachable!("a LizardTail has LizardTail data");
        };
        data.was_used = true;
        s.heal(Decimal::ONE.max(Decimal::from(s.max_hp) * HEAL_FRACTION));
    }
}

#[sts2_macros::hooks]
impl Hooks for LizardTail {
    type Me = RelicId;

    fn should_die_late(&self, s: &CombatState, me: RelicId, creature: CreatureId) -> bool {
        if creature != s.player.creature {
            return true;
        }
        let RelicData::LizardTail(data) = s.relic(me).data else {
            unreachable!("a LizardTail has LizardTail data");
        };
        data.was_used
    }

    fn after_preventing_death(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        let RelicData::LizardTail(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a LizardTail has LizardTail data");
        };
        data.was_used = true;
        let max_hp = Decimal::from(eng.s.creature(creature).max_hp);
        eng.heal(creature, Decimal::ONE.max(max_hp * HEAL_FRACTION))
    }
}
