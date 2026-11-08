//! `VigorPower` (`Models.Powers/VigorPower.cs`): a buff (`Counter`) adding `Amount` to the owner's
//! powered attacks. The first such attack from a card (or with no model source) is remembered in
//! the instance data; once it finishes, `Amount` drops by the amount it had when that attack
//! started. The remembered attack is never cleared, so later attacks from another card get no
//! bonus from the same instance.

use crate::combat::commands::damage::DamageResult;
use crate::combat::commands::{Attack, AttackId};
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

/// Extra damage on the next attack.
pub struct Vigor;

/// The `AttackCommand` a Vigor instance modifies (`commandToModify`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Command {
    pub id: AttackId,
    pub attacker: CreatureId,
    /// `ModelSource as CardModel`.
    pub card: Option<CardId>,
}

/// `VigorPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `commandToModify`.
    pub command_to_modify: Option<Command>,
    /// `amountWhenAttackStarted`.
    pub amount_when_attack_started: i32,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Vigor(data) => data,
        _ => unreachable!("a Vigor instance holds Vigor data"),
    }
}

impl PowerModel for Vigor {
    fn name(&self) -> &'static str {
        "Vigor"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Vigor(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Vigor {
    type Me = PowerRef;

    /// An attack's `ModelSource` is its card or nothing (`FromMonster` leaves it null), so the
    /// `ModelSource is CardModel` guard never skips: a monster's attack is remembered like a card's.
    fn before_attack(&self, eng: &mut Engine<'_>, me: PowerRef, attack: &Attack) -> Flow {
        if attack.attacker != me.owner || !attack.props.is_powered_attack() {
            return Ok(());
        }
        if data(eng.s, me).command_to_modify.is_some() {
            return Ok(());
        }
        let amount_when_attack_started = eng.s.power(me.owner, me.id).amount;
        eng.s.set_power_data(
            me.id,
            PowerData::Vigor(Data {
                command_to_modify: Some(Command {
                    id: attack.id.expect("an executing attack has an id"),
                    attacker: attack.attacker,
                    card: attack.card,
                }),
                amount_when_attack_started,
            }),
        );
        Ok(())
    }

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.dealer != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ZERO;
        }
        if let Some(command) = data(s, me).command_to_modify {
            if query.card.is_some() && query.card != command.card {
                return Decimal::ZERO;
            }
            if Some(command.attacker) != query.dealer {
                return Decimal::ZERO;
            }
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }

    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        attack: &Attack,
        _results: &[DamageResult],
    ) -> Flow {
        let data = data(eng.s, me);
        if data.command_to_modify.map(|c| c.id) == attack.id {
            let offset = Decimal::from(-data.amount_when_attack_started);
            eng.modify_power_amount(me.owner, me.id, offset, None)?;
        }
        Ok(())
    }
}
