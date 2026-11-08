//! `Brimstone` (`Models.Relics/Brimstone.cs`): at the start of each of the owner's turns it gains
//! 2 Strength and every living enemy gains 1.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>("SelfStrength", 2m)`.
const SELF_STRENGTH: Decimal = Decimal::new(2, 0);
/// `PowerVar<StrengthPower>("EnemyStrength", 1m)`.
const ENEMY_STRENGTH: Decimal = Decimal::ONE;

pub struct Brimstone;

impl RelicModel for Brimstone {
    fn name(&self) -> &'static str {
        "Brimstone"
    }
}

#[sts2_macros::hooks]
impl Hooks for Brimstone {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let player = eng.s.player.creature;
        if !participants.contains(&player) {
            return Ok(());
        }
        eng.apply_power(player, PowerKind::Strength, SELF_STRENGTH, Some(player))?;
        let enemies: Vec<CreatureId> = eng
            .s
            .creatures_on(Side::Enemy)
            .into_iter()
            .filter(|&c| eng.s.creature(c).is_alive())
            .collect();
        eng.apply_power_to_all(&enemies, PowerKind::Strength, ENEMY_STRENGTH, None)
    }
}
