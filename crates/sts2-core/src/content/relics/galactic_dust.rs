//! `GalacticDust` (`Models.Relics/GalacticDust.cs`): counts the stars the owner spends; every
//! 10 stars gain 10 unpowered block (more at once if a play spent several tens), keeping the
//! remainder. The count (`[SavedProperty] StarsSpent`) carries over between combats.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `StarsVar(10)`.
const STARS: i32 = 10;
/// `BlockVar(10m, ValueProp.Unpowered)`.
const BLOCK: i32 = 10;

pub struct GalacticDust;

/// `GalacticDust`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `StarsSpent`.
    pub stars_spent: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::GalacticDust(data) = &mut s.relic_mut(me).data else {
        unreachable!("a GalacticDust has GalacticDust data");
    };
    data
}

impl RelicModel for GalacticDust {
    fn name(&self) -> &'static str {
        "Galactic Dust"
    }

    fn init_data(&self) -> RelicData {
        RelicData::GalacticDust(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for GalacticDust {
    type Me = RelicId;

    fn after_stars_spent(&self, eng: &mut Engine<'_>, me: RelicId, amount: i32) -> Flow {
        // `spender == Owner`: the only player.
        let data = data_mut(eng.s, me);
        data.stars_spent += amount;
        let spent = data.stars_spent;
        if spent >= STARS {
            let player = eng.s.player.creature;
            let block = Decimal::from(spent / STARS * BLOCK);
            eng.gain_block(player, block, ValueProps::UNPOWERED, None)?;
            data_mut(eng.s, me).stars_spent %= STARS;
        }
        Ok(())
    }
}
