//! `JossPaper` (`Models.Relics/JossPaper.cs`): draws a card for every 5 cards the owner exhausts.
//! Cards exhausted by Ethereal at turn end are counted after the flush. The count
//! (`[SavedProperty] CardsExhausted`) carries over between combats.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `DynamicVar("ExhaustAmount", 5m)`.
const EXHAUST_AMOUNT: i32 = 5;

pub struct JossPaper;

/// `JossPaper`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CardsExhausted`.
    pub cards_exhausted: i32,
    /// `EtherealCount`.
    pub ethereal_count: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::JossPaper(data) = &mut s.relic_mut(me).data else {
        unreachable!("a JossPaper has JossPaper data");
    };
    data
}

impl RelicModel for JossPaper {
    fn name(&self) -> &'static str {
        "Joss Paper"
    }

    fn init_data(&self) -> RelicData {
        RelicData::JossPaper(Data::default())
    }
}

/// `DrawIfThresholdMet`.
fn draw_if_threshold_met(eng: &mut Engine<'_>, me: RelicId) -> Flow {
    let data = data_mut(eng.s, me);
    if data.cards_exhausted < EXHAUST_AMOUNT {
        return Ok(());
    }
    let count = data.cards_exhausted / EXHAUST_AMOUNT;
    eng.draw(Decimal::from(count), false)?;
    data_mut(eng.s, me).cards_exhausted %= EXHAUST_AMOUNT;
    Ok(())
}

#[sts2_macros::hooks]
impl Hooks for JossPaper {
    type Me = RelicId;

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _card: CardId,
        caused_by_ethereal: bool,
    ) -> Flow {
        // The only player owns every card.
        let data = data_mut(eng.s, me);
        if caused_by_ethereal {
            data.ethereal_count += 1;
            return Ok(());
        }
        data.cards_exhausted += 1;
        draw_if_threshold_met(eng, me)
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            let data = data_mut(eng.s, me);
            data.cards_exhausted += data.ethereal_count;
            data.ethereal_count = 0;
            draw_if_threshold_met(eng, me)?;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).ethereal_count = 0;
        Ok(())
    }
}
