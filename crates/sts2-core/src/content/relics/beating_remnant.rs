//! `BeatingRemnant` (`Models.Relics/BeatingRemnant.cs`): the owner loses at most 20 HP from damage
//! in a turn.

use crate::combat::hooks::{DamageEvent, Hooks, HpLossQuery};
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `DynamicVar("MaxHpLoss", 20m)`.
const MAX_HP_LOSS: i32 = 20;

pub struct BeatingRemnant;

/// `BeatingRemnant`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DamageReceivedThisTurn`.
    pub damage_received_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::BeatingRemnant(data) = &mut s.relic_mut(me).data else {
        unreachable!("a BeatingRemnant has BeatingRemnant data");
    };
    data
}

impl RelicModel for BeatingRemnant {
    fn name(&self) -> &'static str {
        "Beating Remnant"
    }

    fn init_data(&self) -> RelicData {
        RelicData::BeatingRemnant(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for BeatingRemnant {
    type Me = RelicId;

    fn modify_hp_lost_after_osty(
        &self,
        s: &CombatState,
        me: RelicId,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        if !s.in_progress || query.target != s.player.creature {
            return amount;
        }
        let RelicData::BeatingRemnant(data) = s.relic(me).data else {
            unreachable!("a BeatingRemnant has BeatingRemnant data");
        };
        amount.min(Decimal::from(MAX_HP_LOSS - data.damage_received_this_turn))
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: RelicId, hit: DamageEvent) -> Flow {
        if eng.s.in_progress && hit.target == eng.s.player.creature {
            data_mut(eng.s, me).damage_received_this_turn += hit.result.unblocked;
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
            data_mut(eng.s, me).damage_received_this_turn = 0;
        }
        Ok(())
    }
}
