//! `WitheringPresencePower` (`Models.Powers/WitheringPresencePower.cs`): an `Instanced` buff
//! (`Counter`, amount 6) that `Aeonglass` puts on itself aimed at each opponent (`Target`). Every
//! card the target's player plays counts its `CardsLeft` var down; at 0 a `Wither` goes into
//! that player's hand and `CardsLeft` is back to 6 (`DisplayAmount` shows `CardsLeft`). An
//! instance aimed at a pet (Osty, no `Player`) never counts.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct WitheringPresence;

/// `_baseCardsLeft`, also the canonical `CardsLeft`.
pub const BASE_CARDS_LEFT: i32 = 6;

/// The instance's fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `Target`.
    pub target: Option<CreatureId>,
    /// `DynamicVars["CardsLeft"].BaseValue`.
    pub cards_left: i32,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            target: None,
            cards_left: BASE_CARDS_LEFT,
        }
    }
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::WitheringPresence(data) => data,
        _ => unreachable!("a WitheringPresence instance holds WitheringPresence data"),
    }
}

fn set_cards_left(s: &mut CombatState, me: PowerRef, cards_left: i32) {
    let target = data(s, me).target;
    s.set_power_data(
        me.id,
        PowerData::WitheringPresence(Data { target, cards_left }),
    );
}

/// `Aeonglass.AfterAddedToRoom`'s apply for one opponent: a new instance with `Target` set,
/// applied to `owner` with `amount`.
pub fn apply(eng: &mut Engine<'_>, owner: CreatureId, target: CreatureId, amount: Decimal) -> Flow {
    let instance =
        eng.apply_power_instance(owner, PowerKind::WitheringPresence, amount, Some(owner))?;
    if let Some(me) = instance {
        eng.s.set_power_data(
            me.id,
            PowerData::WitheringPresence(Data {
                target: Some(target),
                ..Data::default()
            }),
        );
    }
    Ok(())
}

impl PowerModel for WitheringPresence {
    fn name(&self) -> &'static str {
        "Withering Presence"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::WitheringPresence(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for WitheringPresence {
    type Me = PowerRef;

    /// `cardPlay.Card.Owner == Target.Player`: every card is the player's, so the target must be
    /// the player creature.
    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        let Data { target, cards_left } = data(eng.s, me);
        let Some(target) = target.filter(|&t| eng.s.creature(t).is_player()) else {
            return Ok(());
        };
        let cards_left = cards_left - 1;
        set_cards_left(eng.s, me, cards_left);
        if cards_left <= 0 {
            eng.add_to_combat_and_preview(target, CardKind::Wither, PileType::Hand, 1, None)?;
            set_cards_left(eng.s, me, BASE_CARDS_LEFT);
        }
        Ok(())
    }
}
