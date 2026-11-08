//! `LavaLamp` (`Models.Relics/LavaLamp.cs`): a fight in which the owner takes no HP damage (from a
//! blockable source) offers its card rewards upgraded (`TryModifyCardRewardOptionsLate`). The
//! flag (`[SavedProperty] TookDamageThisCombat`) is reset in every room entered.

use crate::combat::hooks::{DamageEvent, Hooks};
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{CardCreation, Room, RoomType, RunCard, RunState};

pub struct LavaLamp;

/// `LavaLamp`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TookDamageThisCombat`.
    pub took_damage_this_combat: bool,
}

impl RelicModel for LavaLamp {
    fn name(&self) -> &'static str {
        "Lava Lamp"
    }

    fn init_data(&self) -> RelicData {
        RelicData::LavaLamp(Data::default())
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, me: usize, _room: RoomType) {
        s.relics[me].data = RelicData::LavaLamp(Data::default());
    }

    fn try_modify_card_reward_options_late(
        &self,
        s: &mut RunState,
        me: usize,
        cards: &mut Vec<RunCard>,
        _options: CardCreation,
    ) -> bool {
        let RelicData::LavaLamp(data) = s.relics[me].data else {
            unreachable!("a LavaLamp has LavaLamp data");
        };
        if !matches!(s.room, Room::Combat { .. }) || data.took_damage_this_combat {
            return false;
        }
        for card in cards.iter_mut().filter(|c| c.is_upgradable()) {
            card.upgrade();
        }
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for LavaLamp {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        eng.s.relic_mut(me).data = RelicData::LavaLamp(Data::default());
        Ok(())
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: RelicId, hit: DamageEvent) -> Flow {
        if hit.target == eng.s.player.creature
            && hit.result.unblocked > 0
            && !hit.props.has(ValueProps::UNBLOCKABLE)
        {
            eng.s.relic_mut(me).data = RelicData::LavaLamp(Data {
                took_damage_this_combat: true,
            });
        }
        Ok(())
    }
}
