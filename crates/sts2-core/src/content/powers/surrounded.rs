//! `SurroundedPower` (`Models.Powers/SurroundedPower.cs`): a debuff (`Single`) the `Rocket` puts on
//! every opponent. Its owner faces one crab arm (`Facing`, `Right` at first); an attacker behind it
//! (`BackAttackLeftPower` while facing right, `BackAttackRightPower` while facing left) deals 1.5×
//! damage. The owner turns to face the arm a card play or potion of its player targets, and, when
//! an enemy dies, to face the remaining arms if they are all on one side. Turning is otherwise
//! visual (`FaceDirection`'s sprite flips and music).

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, Potion};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// Back attacks deal 1.5× damage.
pub struct Surrounded;

/// `SurroundedPower.Direction`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    #[default]
    Right,
    Left,
}

/// The instance's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_facing`.
    pub facing: Direction,
}

fn facing(s: &CombatState, me: PowerRef) -> Direction {
    match *s.power_data(me.id) {
        PowerData::Surrounded(data) => data.facing,
        _ => unreachable!("a Surrounded instance holds Surrounded data"),
    }
}

/// `UpdateDirection(target)`: turn around when `target` is behind.
fn update_direction(s: &mut CombatState, me: PowerRef, target: CreatureId) {
    let behind = match facing(s, me) {
        Direction::Right => {
            (s.creature(target).has_power(PowerKind::BackAttackLeft)).then_some(Direction::Left)
        }
        Direction::Left => {
            (s.creature(target).has_power(PowerKind::BackAttackRight)).then_some(Direction::Right)
        }
    };
    if let Some(direction) = behind
        && let PowerData::Surrounded(data) = s.power_data_mut(me.id)
    {
        data.facing = direction;
    }
}

impl PowerModel for Surrounded {
    fn name(&self) -> &'static str {
        "Surrounded"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Surrounded(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Surrounded {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let Some(dealer) = query.dealer else {
            return Decimal::ONE;
        };
        if query.target != Some(me.owner) {
            return Decimal::ONE;
        }
        let back = match facing(s, me) {
            Direction::Right => PowerKind::BackAttackLeft,
            Direction::Left => PowerKind::BackAttackRight,
        };
        if s.creature(dealer).has_power(back) {
            Decimal::new(15, 1)
        } else {
            Decimal::ONE
        }
    }

    /// `cardPlay.Card.Owner == Owner.Player`: a pet owner has no `Player`.
    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if let Some(target) = play.target
            && eng.s.creature(me.owner).is_player()
        {
            update_direction(eng.s, me, target);
        }
        Ok(())
    }

    /// `potion.Owner == Owner.Player`: a pet owner has no `Player`.
    fn before_potion_used(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _potion: Potion,
        target: Option<CreatureId>,
    ) -> Flow {
        if let Some(target) = target
            && eng.s.in_progress
            && eng.s.creature(me.owner).is_player()
        {
            update_direction(eng.s, me, target);
        }
        Ok(())
    }

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if was_removal_prevented || eng.s.creature(creature).side == eng.s.creature(me.owner).side {
            return Ok(());
        }
        let hittable = eng.hooked().hittable_enemies();
        let all_have = |kind| hittable.iter().all(|&e| eng.s.creature(e).has_power(kind));
        if let Some(&first) = hittable.first()
            && (all_have(PowerKind::BackAttackLeft) || all_have(PowerKind::BackAttackRight))
        {
            update_direction(eng.s, me, first);
        }
        Ok(())
    }
}
