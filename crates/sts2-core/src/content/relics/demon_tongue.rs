//! `DemonTongue` (`Models.Relics/DemonTongue.cs`): the first time each turn the owner loses HP on
//! its own side's turn, it heals that much. Reset at the start of the owner's side's turn.

use crate::combat::hooks::{DamageEvent, Hooks};
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

pub struct DemonTongue;

/// `DemonTongue`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_triggeredThisTurn`.
    pub triggered_this_turn: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::DemonTongue(data) = &mut s.relic_mut(me).data else {
        unreachable!("a DemonTongue has DemonTongue data");
    };
    data
}

impl RelicModel for DemonTongue {
    fn name(&self) -> &'static str {
        "Demon Tongue"
    }

    fn init_data(&self) -> RelicData {
        RelicData::DemonTongue(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for DemonTongue {
    type Me = RelicId;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: RelicId, hit: DamageEvent) -> Flow {
        let player = eng.s.player.creature;
        if eng.s.side != eng.s.creature(player).side
            || hit.target != player
            || hit.result.unblocked <= 0
            || data_mut(eng.s, me).triggered_this_turn
        {
            return Ok(());
        }
        data_mut(eng.s, me).triggered_this_turn = true;
        eng.heal(player, Decimal::from(hit.result.unblocked))?;
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
            data_mut(eng.s, me).triggered_this_turn = false;
        }
        Ok(())
    }
}
