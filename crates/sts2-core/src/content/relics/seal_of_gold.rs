//! `SealOfGold` (`Models.Relics/SealOfGold.cs`): Ancient (Tezcatara); at the start of each of the
//! owner's turns, if it has at least 3 gold, 1 energy, then it loses 3 gold.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `GoldVar(3)`.
const GOLD: i32 = 3;

pub struct SealOfGold;

impl RelicModel for SealOfGold {
    fn name(&self) -> &'static str {
        "Seal of Gold"
    }
}

#[sts2_macros::hooks]
impl Hooks for SealOfGold {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.gold >= GOLD {
            eng.gain_energy(ENERGY)?;
            // `PlayerCmd.LoseGold(3, Owner)`: the map point's `GoldLost` is run history only.
            eng.lose_gold(Decimal::from(GOLD));
        }
        Ok(())
    }
}
