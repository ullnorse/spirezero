//! `CurlUpPower` (`Models.Powers/CurlUpPower.cs`): a buff (`Counter`, the block) on
//! `LouseProgenitor`. The first card to hurt the owner with a powered attack is remembered; once
//! that card has been played the owner gains `Amount` unpowered block and the power is removed.
//! `Curled` and the animation are cosmetic.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, ValueProps};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct CurlUp;

/// `CurlUpPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `playedCard`.
    pub played_card: Option<CardId>,
}

fn played_card(s: &CombatState, me: PowerRef) -> Option<CardId> {
    match *s.power_data(me.id) {
        PowerData::CurlUp(data) => data.played_card,
        _ => unreachable!("a CurlUp instance holds CurlUp data"),
    }
}

fn set_played_card(eng: &mut Engine<'_>, me: PowerRef, card: Option<CardId>) {
    eng.s
        .set_power_data(me.id, PowerData::CurlUp(Data { played_card: card }));
}

impl PowerModel for CurlUp {
    fn name(&self) -> &'static str {
        "Curl Up"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::CurlUp(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for CurlUp {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || !hit.props.is_powered_attack() {
            return Ok(());
        }
        let Some(card) = hit.card else {
            return Ok(());
        };
        if played_card(eng.s, me).is_some_and(|played| played != card) {
            return Ok(());
        }
        set_played_card(eng, me, Some(card));
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if played_card(eng.s, me) != Some(play.card) {
            return Ok(());
        }
        set_played_card(eng, me, None);
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        eng.remove_power(me.owner, me.id)
    }
}
