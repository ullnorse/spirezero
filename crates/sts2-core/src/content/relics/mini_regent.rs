//! `MiniRegent` (`Models.Relics/MiniRegent.cs`): the first time each turn the owner spends
//! stars, gain 1 Strength.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: Decimal = Decimal::ONE;

pub struct MiniRegent;

/// `MiniRegent`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `UsedThisTurn`.
    pub used_this_turn: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::MiniRegent(data) = &mut s.relic_mut(me).data else {
        unreachable!("a MiniRegent has MiniRegent data");
    };
    data
}

impl RelicModel for MiniRegent {
    fn name(&self) -> &'static str {
        "Mini Regent"
    }

    fn init_data(&self) -> RelicData {
        RelicData::MiniRegent(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for MiniRegent {
    type Me = RelicId;

    fn after_stars_spent(&self, eng: &mut Engine<'_>, me: RelicId, _amount: i32) -> Flow {
        // `spender == Owner`: the only player.
        let data = data_mut(eng.s, me);
        if data.used_this_turn {
            return Ok(());
        }
        data.used_this_turn = true;
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Strength, STRENGTH, Some(player))
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
