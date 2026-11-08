//! `DaughterOfTheWind` (`Models.Relics/DaughterOfTheWind.cs`): every Attack the owner plays gives
//! 1 Block. Event rarity (`DollRoom`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(1m, ValueProp.Unpowered)`.
const BLOCK: i32 = 1;

pub struct DaughterOfTheWind;

impl RelicModel for DaughterOfTheWind {
    fn name(&self) -> &'static str {
        "Daughter of the Wind"
    }
}

#[sts2_macros::hooks]
impl Hooks for DaughterOfTheWind {
    type Me = RelicId;

    /// `AfterCardPlayed`: an Attack of the owner's (every card is the player's).
    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        if eng.s.card(play.card).card_type() == CardType::Attack {
            let owner = eng.s.player.creature;
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
