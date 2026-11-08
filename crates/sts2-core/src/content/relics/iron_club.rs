//! `IronClub` (`Models.Relics/IronClub.cs`): Ancient (Tanx); draws a card after every 4th card
//! the owner plays (`CardsPlayed` is saved, so it counts across combats). `_isActivating` and the
//! status are display only.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(4)`.
const CARDS: i32 = 4;

pub struct IronClub;

/// `IronClub`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CardsPlayed`.
    pub cards_played: i32,
}

impl RelicModel for IronClub {
    fn name(&self) -> &'static str {
        "Iron Club"
    }

    fn init_data(&self) -> RelicData {
        RelicData::IronClub(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for IronClub {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, _play: &CardPlay) -> Flow {
        let RelicData::IronClub(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("an IronClub has IronClub data");
        };
        data.cards_played += 1;
        let played = data.cards_played;
        if eng.s.in_progress && played % CARDS == 0 {
            eng.draw(Decimal::ONE, false)?;
        }
        Ok(())
    }
}
