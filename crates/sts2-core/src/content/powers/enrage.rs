//! `EnragePower` (`Models.Powers/EnragePower.cs`): a buff (`Counter`) on `TestSubject`. After any
//! Skill is played, its owner gains `Amount` Strength.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow};
use crate::content::cards::CardType;
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Enrage;

impl PowerModel for Enrage {
    fn name(&self) -> &'static str {
        "Enrage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Enrage {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if eng.s.card(play.card).card_type() == CardType::Skill {
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.apply_power(
                me.owner,
                PowerKind::Strength,
                Decimal::from(amount),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
