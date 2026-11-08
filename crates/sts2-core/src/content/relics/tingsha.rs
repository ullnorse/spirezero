//! `Tingsha` (`Models.Relics/Tingsha.cs`): each card discarded by a card effect on the owner's
//! side's turn deals 3 unpowered damage to a random hittable enemy (`Rng.CombatTargets`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(3m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 3;

pub struct Tingsha;

impl RelicModel for Tingsha {
    fn name(&self) -> &'static str {
        "Tingsha"
    }
}

#[sts2_macros::hooks]
impl Hooks for Tingsha {
    type Me = RelicId;

    fn after_card_discarded(&self, eng: &mut Engine<'_>, _me: RelicId, _card: CardId) -> Flow {
        let owner = eng.s.player.creature;
        if eng.s.creature(owner).side != eng.s.side {
            return Ok(());
        }
        // Flash: visual only.
        let enemies = eng.hooked().hittable_enemies();
        let Some(target) = eng.random_target(&enemies) else {
            return Ok(());
        };
        eng.damage(
            &[target],
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(owner),
            None,
        )?;
        Ok(())
    }
}
