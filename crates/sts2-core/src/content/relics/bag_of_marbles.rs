//! `BagOfMarbles` (`Models.Relics/BagOfMarbles.cs`): 1 Vulnerable to every hittable enemy before
//! the first player turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<VulnerablePower>(1m)`.
const VULNERABLE: i32 = 1;

pub struct BagOfMarbles;

impl RelicModel for BagOfMarbles {
    fn name(&self) -> &'static str {
        "Bag of Marbles"
    }
}

#[sts2_macros::hooks]
impl Hooks for BagOfMarbles {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number <= 1 {
            let targets = eng.hooked().hittable_enemies();
            eng.apply_power_to_all(
                &targets,
                PowerKind::Vulnerable,
                Decimal::from(VULNERABLE),
                Some(owner),
            )?;
        }
        Ok(())
    }
}
