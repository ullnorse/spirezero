//! `BurningSticks` (`Models.Relics/BurningSticks.cs`): the first Skill the owner exhausts each
//! fight is cloned into the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, Engine, Flow, PileType, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};

pub struct BurningSticks;

/// `BurningSticks`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `WasUsedThisCombat`.
    pub was_used_this_combat: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::BurningSticks(data) = &mut s.relic_mut(me).data else {
        unreachable!("a BurningSticks has BurningSticks data");
    };
    data
}

impl RelicModel for BurningSticks {
    fn name(&self) -> &'static str {
        "Burning Sticks"
    }

    fn init_data(&self) -> RelicData {
        RelicData::BurningSticks(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for BurningSticks {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).was_used_this_combat = false;
        Ok(())
    }

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        card: CardId,
        _caused_by_ethereal: bool,
    ) -> Flow {
        // The only player owns every card.
        if data_mut(eng.s, me).was_used_this_combat
            || eng.s.card(card).card_type() != CardType::Skill
        {
            return Ok(());
        }
        let clone = eng.create_clone(card);
        let owner = eng.s.player.creature;
        eng.add_generated_card_to_combat(clone, PileType::Hand, Some(owner))?;
        data_mut(eng.s, me).was_used_this_combat = true;
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).was_used_this_combat = false;
        Ok(())
    }
}
