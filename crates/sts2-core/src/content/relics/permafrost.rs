//! `Permafrost` (`Models.Relics/Permafrost.cs`): 7 Block the first time the owner plays a Power
//! each fight.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, Engine, Flow, RelicId, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `BlockVar(7m, ValueProp.Unpowered)`.
const BLOCK: i32 = 7;

pub struct Permafrost;

/// `Permafrost`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `ActivatedThisCombat`.
    pub activated_this_combat: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Permafrost(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Permafrost has Permafrost data");
    };
    data
}

impl RelicModel for Permafrost {
    fn name(&self) -> &'static str {
        "Permafrost"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Permafrost(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Permafrost {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).activated_this_combat = false;
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress
            || eng.s.card(play.card).card_type() != CardType::Power
            || data_mut(eng.s, me).activated_this_combat
        {
            return Ok(());
        }
        let owner = eng.s.player.creature;
        eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        data_mut(eng.s, me).activated_this_combat = true;
        Ok(())
    }
}
