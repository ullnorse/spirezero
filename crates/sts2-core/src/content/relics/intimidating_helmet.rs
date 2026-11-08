//! `IntimidatingHelmet` (`Models.Relics/IntimidatingHelmet.cs`): 4 Block before the owner plays a
//! card that costs at least 2 energy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(4m, ValueProp.Unpowered)`.
const BLOCK: i32 = 4;
/// `EnergyVar(2)` (`IntValue`).
const ENERGY: i32 = 2;

pub struct IntimidatingHelmet;

impl RelicModel for IntimidatingHelmet {
    fn name(&self) -> &'static str {
        "Intimidating Helmet"
    }
}

#[sts2_macros::hooks]
impl Hooks for IntimidatingHelmet {
    type Me = RelicId;

    fn before_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        // The only player owns every card.
        if play.energy_value >= ENERGY {
            let owner = eng.s.player.creature;
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
