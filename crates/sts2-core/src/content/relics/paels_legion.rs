//! `PaelsLegion` (`Models.Relics/PaelsLegion.cs`): Ancient (Pael); adds the `PaelsLegion` pet to
//! every combat (`BeforeCombatStart`; `AddsPet`). Block from the owner's card moves is doubled
//! while `Cooldown <= 0`; the first card play whose block it changed sets `Cooldown` to 2 once
//! that play finishes, and every turn start of the owner takes 1 off (it goes below 0). The skin
//! (`new Rng(Owner, Id, 0)`, a fresh stream) and the pet's animations are cosmetic.

use crate::combat::hooks::{BlockQuery, Hooks};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::monsters::{self, MonsterKind};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `DynamicVar("Turns", 2m)`.
const TURNS: i32 = 2;

pub struct PaelsLegion;

/// `PaelsLegion`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_cooldown`.
    pub cooldown: i32,
    /// `_triggeredBlockLastTurn`.
    pub triggered_block_last_turn: bool,
    /// `_affectedCardPlay`, by [`CardPlay::id`].
    pub affected_card_play: Option<u32>,
}

fn data(s: &CombatState, me: RelicId) -> Data {
    let RelicData::PaelsLegion(data) = s.relic(me).data else {
        unreachable!("a PaelsLegion has PaelsLegion data");
    };
    data
}

fn data_mut<'a>(eng: &'a mut Engine<'_>, me: RelicId) -> &'a mut Data {
    let RelicData::PaelsLegion(data) = &mut eng.s.relic_mut(me).data else {
        unreachable!("a PaelsLegion has PaelsLegion data");
    };
    data
}

impl RelicModel for PaelsLegion {
    fn name(&self) -> &'static str {
        "Pael's Legion"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PaelsLegion(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsLegion {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `SummonPet`: `PlayerCmd.AddPet<PaelsLegion>(Owner)`.
        eng.add_pet(MonsterKind::PaelsLegion(monsters::PaelsLegion));
        Ok(())
    }

    fn modify_block_multiplicative(
        &self,
        s: &CombatState,
        me: RelicId,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        // `cardSource.Owner != Owner`: the only player owns every card.
        if !query.props.is_card_or_monster_move() || query.card.is_none() {
            return Decimal::ONE;
        }
        if data(s, me).cooldown > 0 {
            return Decimal::ONE;
        }
        Decimal::new(2, 0)
    }

    fn after_modifying_block_amount(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        amount: Decimal,
        _card: Option<CardId>,
        play: Option<&CardPlay>,
    ) -> Flow {
        let Some(play) = play.filter(|_| amount > Decimal::ZERO) else {
            return Ok(());
        };
        let data = data_mut(eng, me);
        if data.affected_card_play.is_some_and(|id| id != play.id) {
            return Ok(());
        }
        data.affected_card_play = Some(play.id);
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        let data = data_mut(eng, me);
        if data.affected_card_play == Some(play.id) {
            data.affected_card_play = None;
            data.cooldown = TURNS;
            data.triggered_block_last_turn = true;
        }
        Ok(())
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            let data = data_mut(eng, me);
            data.cooldown -= 1;
            data.triggered_block_last_turn = false;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng, me) = Data::default();
        Ok(())
    }
}
