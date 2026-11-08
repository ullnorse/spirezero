//! `Vambrace` (`Models.Relics/Vambrace.cs`): the first Block the owner gains from a card each
//! fight is doubled. The card that gained it is remembered until the card finishes.

use crate::combat::hooks::{BlockQuery, Hooks};
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

pub struct Vambrace;

/// `Vambrace`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TriggeringCard`.
    pub triggering_card: Option<CardId>,
    /// `BlockGainedThisCombat`.
    pub block_gained_this_combat: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Vambrace(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Vambrace has Vambrace data");
    };
    data
}

impl RelicModel for Vambrace {
    fn name(&self) -> &'static str {
        "Vambrace"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Vambrace(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Vambrace {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }

    fn modify_block_multiplicative(
        &self,
        s: &CombatState,
        me: RelicId,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        // `IsCardOrMonsterMove`; the only player owns every card.
        let Some(card) = query.card else {
            return Decimal::ONE;
        };
        if !query.props.has(ValueProps::MOVE) {
            return Decimal::ONE;
        }
        let RelicData::Vambrace(data) = s.relic(me).data else {
            unreachable!("a Vambrace has Vambrace data");
        };
        if data.triggering_card.is_some_and(|t| t != card) || data.block_gained_this_combat {
            return Decimal::ONE;
        }
        Decimal::from(2)
    }

    fn after_modifying_block_amount(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        amount: Decimal,
        card: Option<CardId>,
        _play: Option<&CardPlay>,
    ) -> Flow {
        if let Some(card) = card.filter(|_| amount > Decimal::ZERO) {
            data_mut(eng.s, me).triggering_card = Some(card);
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        let data = data_mut(eng.s, me);
        if data.triggering_card == Some(play.card) && !data.block_gained_this_combat {
            data.block_gained_this_combat = true;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }
}
