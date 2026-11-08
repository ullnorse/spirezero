//! `ThrowingAxe` (`Models.Relics/ThrowingAxe.cs`): Ancient (Tanx); the first card the owner plays
//! each combat is played one extra time. The flag resets on entering a fight and when it ends.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};

pub struct ThrowingAxe;

/// `ThrowingAxe`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `UsedThisCombat`.
    pub used_this_combat: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::ThrowingAxe(data) = &mut s.relic_mut(me).data else {
        unreachable!("a ThrowingAxe has ThrowingAxe data");
    };
    data
}

impl RelicModel for ThrowingAxe {
    fn name(&self) -> &'static str {
        "Throwing Axe"
    }

    fn init_data(&self) -> RelicData {
        RelicData::ThrowingAxe(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for ThrowingAxe {
    type Me = RelicId;

    // The combat only dispatches this for its own room (`room is CombatRoom`).
    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).used_this_combat = false;
        Ok(())
    }

    // Every card belongs to the only player.
    fn modify_card_play_count(
        &self,
        s: &CombatState,
        me: RelicId,
        _card: CardId,
        _target: Option<CreatureId>,
        count: u32,
    ) -> u32 {
        let RelicData::ThrowingAxe(data) = s.relic(me).data else {
            unreachable!("a ThrowingAxe has ThrowingAxe data");
        };
        if data.used_this_combat {
            count
        } else {
            count + 1
        }
    }

    fn after_modifying_card_play_count(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _card: CardId,
    ) -> Flow {
        data_mut(eng.s, me).used_this_combat = true;
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).used_this_combat = false;
        Ok(())
    }
}
