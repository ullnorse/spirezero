//! `BeltBuckle` (`Models.Relics/BeltBuckle.cs`): 2 Dexterity while the owner holds no potion.
//! Obtaining it mid-combat (`AfterObtained`) has no combat path here.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, Potion, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `PowerVar<DexterityPower>(2m)`.
const DEXTERITY: i32 = 2;

pub struct BeltBuckle;

/// `BeltBuckle`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DexterityApplied`.
    pub dexterity_applied: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::BeltBuckle(data) = &mut s.relic_mut(me).data else {
        unreachable!("a BeltBuckle has BeltBuckle data");
    };
    data
}

/// `Owner.Potions.Any()`: an unported potion still holds its slot.
fn any_potions(s: &CombatState) -> bool {
    s.player.potions.iter().any(Option::is_some)
        || s.player.unported_potions.iter().any(Option::is_some)
}

/// `ApplyDexterity`.
fn apply_dexterity(eng: &mut Engine<'_>, me: RelicId) -> Flow {
    let data = data_mut(eng.s, me);
    if !data.dexterity_applied {
        data.dexterity_applied = true;
        let owner = eng.s.player.creature;
        eng.apply_power(owner, PowerKind::Dexterity, Decimal::from(DEXTERITY), None)?;
    }
    Ok(())
}

/// `RemoveDexterity`.
fn remove_dexterity(eng: &mut Engine<'_>, me: RelicId) -> Flow {
    let data = data_mut(eng.s, me);
    if data.dexterity_applied {
        data.dexterity_applied = false;
        let owner = eng.s.player.creature;
        eng.apply_power(owner, PowerKind::Dexterity, Decimal::from(-DEXTERITY), None)?;
    }
    Ok(())
}

impl RelicModel for BeltBuckle {
    fn name(&self) -> &'static str {
        "Belt Buckle"
    }

    fn init_data(&self) -> RelicData {
        RelicData::BeltBuckle(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for BeltBuckle {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).dexterity_applied = false;
        if !any_potions(eng.s) {
            apply_dexterity(eng, me)?;
        }
        Ok(())
    }

    fn after_potion_procured(&self, eng: &mut Engine<'_>, me: RelicId, _potion: Potion) -> Flow {
        if eng.s.in_progress && any_potions(eng.s) {
            remove_dexterity(eng, me)?;
        }
        Ok(())
    }

    fn after_potion_discarded(&self, eng: &mut Engine<'_>, me: RelicId, _potion: Potion) -> Flow {
        if eng.s.in_progress && !any_potions(eng.s) {
            apply_dexterity(eng, me)?;
        }
        Ok(())
    }

    fn after_potion_used(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _potion: Potion,
        _target: Option<crate::combat::CreatureId>,
    ) -> Flow {
        if eng.s.in_progress && !any_potions(eng.s) {
            apply_dexterity(eng, me)?;
        }
        Ok(())
    }

    fn after_combat_victory(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).dexterity_applied = false;
        Ok(())
    }
}
