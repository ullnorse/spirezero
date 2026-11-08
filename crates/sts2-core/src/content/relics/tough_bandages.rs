//! `ToughBandages` (`Models.Relics/ToughBandages.cs`): each card discarded by a card effect on
//! the owner's side's turn gives 3 unpowered Block.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(3m, ValueProp.Unpowered)`.
const BLOCK: i32 = 3;

pub struct ToughBandages;

impl RelicModel for ToughBandages {
    fn name(&self) -> &'static str {
        "Tough Bandages"
    }
}

#[sts2_macros::hooks]
impl Hooks for ToughBandages {
    type Me = RelicId;

    fn after_card_discarded(&self, eng: &mut Engine<'_>, _me: RelicId, _card: CardId) -> Flow {
        let owner = eng.s.player.creature;
        if eng.s.creature(owner).side != eng.s.side {
            return Ok(());
        }
        // Flash: visual only.
        eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
