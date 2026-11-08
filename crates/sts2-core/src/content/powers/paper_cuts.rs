//! `PaperCutsPower` (`Models.Powers/PaperCutsPower.cs`): a buff (`Counter`) on `ScrollOfBiting`.
//! When the owner's powered attack deals unblocked damage to a player, that player loses
//! `Amount` max HP (`CreatureCmd.LoseMaxHp`, not from a card).

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct PaperCuts;

impl PowerModel for PaperCuts {
    fn name(&self) -> &'static str {
        "Paper Cuts"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

/// `CreatureCmd.LoseMaxHp(creature, amount, isFromCard: false)` (`CreatureCmd.cs:863-880`): HP
/// above the new max is lost as unblockable, unpowered damage, then max HP drops (to at least 1).
/// The map point's `MaxHpLost` is run history only.
fn lose_max_hp(eng: &mut Engine<'_>, creature: CreatureId, amount: Decimal) -> Flow {
    let c = eng.s.creature(creature);
    let new_max = Decimal::from(c.max_hp) - amount;
    let hp = Decimal::from(c.hp);
    if new_max < hp {
        eng.damage(
            &[creature],
            hp - new_max,
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            None,
            None,
        )?;
    }
    eng.set_max_hp(creature, new_max.max(Decimal::ONE))?;
    Ok(())
}

#[sts2_macros::hooks]
impl Hooks for PaperCuts {
    type Me = PowerRef;

    fn after_damage_given(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.dealer != Some(me.owner)
            || !eng.s.creature(hit.target).is_player()
            || !hit.props.is_powered_attack()
            || hit.result.unblocked <= 0
        {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        lose_max_hp(eng, hit.target, amount)
    }
}
