//! `MonologuePower` (`Models.Powers/MonologuePower.cs`): an `Instanced` buff. Each card the owner
//! plays while it is on gives the owner Strength (the instance's `Strength` var, set by the
//! card) once the play finishes; the total (`StrengthApplied`) is taken back at the end of the
//! owner's turn, when the power removes itself. A card that started playing before the power
//! was applied is not counted (`amountsForPlayedCards`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Monologue;

/// Cards whose play is open at once (nested auto-plays).
const OPEN_PLAYS: usize = 8;

/// `MonologuePower.Data` plus the instance's `Strength` (`PowerVar<StrengthPower>(1m)`) and
/// `StrengthApplied` (`DynamicVar("StrengthApplied", 0m)`) vars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DynamicVars.Strength`: written by the card that applies the power.
    pub strength: i32,
    /// `DynamicVars["StrengthApplied"]`.
    pub strength_applied: i32,
    /// `amountsForPlayedCards`: each open play and the Strength it will give.
    pub played: [Option<(CardId, i32)>; OPEN_PLAYS],
}

impl Default for Data {
    fn default() -> Self {
        Self {
            strength: 1,
            strength_applied: 0,
            played: [None; OPEN_PLAYS],
        }
    }
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Monologue(data) => data,
        _ => unreachable!("a Monologue instance holds Monologue data"),
    }
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Monologue(data) => data,
        _ => unreachable!("a Monologue instance holds Monologue data"),
    }
}

/// Sets the Strength var of a freshly applied instance (`monologuePower.DynamicVars.Strength
/// .BaseValue = ..`).
pub fn set_strength(s: &mut CombatState, me: PowerRef, strength: i32) {
    data_mut(s, me).strength = strength;
}

impl PowerModel for Monologue {
    fn name(&self) -> &'static str {
        "Monologue"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Monologue(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Monologue {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature != Owner`: every card belongs to the player.
        let data = data_mut(eng.s, me);
        let strength = data.strength;
        let slot = data
            .played
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("fewer than eight plays open at once");
        *slot = Some((play.card, strength));
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        let data = data_mut(eng.s, me);
        let Some(slot) = data
            .played
            .iter_mut()
            .find(|slot| slot.is_some_and(|(card, _)| card == play.card))
        else {
            return Ok(());
        };
        let (_, value) = slot.take().expect("the slot was just matched");
        eng.apply_power(
            me.owner,
            PowerKind::Strength,
            Decimal::from(value),
            Some(me.owner),
        )?;
        let data = data_mut(eng.s, me);
        data.strength_applied += data.strength;
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let applied = data(eng.s, me).strength_applied;
            eng.remove_power(me.owner, me.id)?;
            eng.apply_power(
                me.owner,
                PowerKind::Strength,
                Decimal::from(-applied),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
