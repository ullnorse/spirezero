//! `Regalite` (`Models.Relics/Regalite.cs`): the first time each turn the owner creates a card for
//! the combat, it gains 4 Unpowered block.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `BlockVar(4m, ValueProp.Unpowered)`.
const BLOCK: i32 = 4;

pub struct Regalite;

/// `Regalite`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_usedThisTurn`.
    pub used_this_turn: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Regalite(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Regalite has Regalite data");
    };
    data
}

impl RelicModel for Regalite {
    fn name(&self) -> &'static str {
        "Regalite"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Regalite(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Regalite {
    type Me = RelicId;

    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        let owner = eng.s.player.creature;
        if creator == Some(owner) && !data_mut(eng.s, me).used_this_turn {
            data_mut(eng.s, me).used_this_turn = true;
            // Flash: visual only; `fast: true` is display.
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            data_mut(eng.s, me).used_this_turn = false;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).used_this_turn = false;
        Ok(())
    }
}
