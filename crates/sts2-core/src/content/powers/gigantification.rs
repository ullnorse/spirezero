//! `GigantificationPower` (`Models.Powers/GigantificationPower.cs`): the owner's next powered
//! Attack card deals triple damage, then one stack is used up. The first such attack is
//! remembered in the instance data (`commandToModify`) until it finishes.

use crate::combat::commands::damage::DamageResult;
use crate::combat::commands::{Attack, AttackId};
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CardId, CombatState, Engine, Flow};
use crate::content::cards::CardType;
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Gigantification;

/// The `AttackCommand` a Gigantification instance modifies (`commandToModify`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Command {
    pub id: AttackId,
    /// `ModelSource as CardModel`.
    pub card: CardId,
}

/// `GigantificationPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub command_to_modify: Option<Command>,
}

/// `TripleDamage`: the multiplier of the modified attack.
const TRIPLE: Decimal = Decimal::new(3, 0);

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Gigantification(data) => data,
        _ => unreachable!("a Gigantification instance holds Gigantification data"),
    }
}

impl PowerModel for Gigantification {
    fn name(&self) -> &'static str {
        "Gigantification"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Gigantification(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Gigantification {
    type Me = PowerRef;

    // Every card belongs to the only player (`cardModel.Owner.Creature != Owner` is a creature test).
    fn before_attack(&self, eng: &mut Engine<'_>, me: PowerRef, attack: &Attack) -> Flow {
        let Some(card) = attack.card else {
            return Ok(());
        };
        if eng.s.player.creature != me.owner
            || eng.s.card(card).card_type() != CardType::Attack
            || !attack.props.is_powered_attack()
            || data(eng.s, me).command_to_modify.is_some()
        {
            return Ok(());
        }
        eng.s.set_power_data(
            me.id,
            PowerData::Gigantification(Data {
                command_to_modify: Some(Command {
                    id: attack.id.expect("an executing attack has an id"),
                    card,
                }),
            }),
        );
        Ok(())
    }

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let Some(card) = query.card else {
            return Decimal::ONE;
        };
        if s.player.creature != me.owner || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        match data(s, me).command_to_modify {
            Some(command) if command.card != card => Decimal::ONE,
            _ => TRIPLE,
        }
    }

    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        attack: &Attack,
        _results: &[DamageResult],
    ) -> Flow {
        if data(eng.s, me).command_to_modify.map(|c| c.id) == attack.id && attack.id.is_some() {
            eng.s
                .set_power_data(me.id, PowerData::Gigantification(Data::default()));
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
