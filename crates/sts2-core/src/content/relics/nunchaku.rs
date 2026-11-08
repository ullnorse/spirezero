//! `Nunchaku` (`Models.Relics/Nunchaku.cs`): every 10th Attack the owner plays gains 1 energy.
//! The count (`[SavedProperty] AttacksPlayed`) carries over between combats. `IsActivating`
//! only drives the counter display.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(10)`.
const CARDS: i32 = 10;
/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct Nunchaku;

/// `Nunchaku`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayed`.
    pub attacks_played: i32,
}

impl RelicModel for Nunchaku {
    fn name(&self) -> &'static str {
        "Nunchaku"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Nunchaku(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Nunchaku {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        // The only player owns every card.
        if eng.s.card(play.card).card_type() != CardType::Attack {
            return Ok(());
        }
        let RelicData::Nunchaku(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a Nunchaku has Nunchaku data");
        };
        data.attacks_played += 1;
        let attacks_played = data.attacks_played;
        if eng.s.in_progress && attacks_played % CARDS == 0 {
            eng.gain_energy(ENERGY)?;
        }
        Ok(())
    }
}
