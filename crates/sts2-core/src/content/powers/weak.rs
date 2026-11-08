//! `WeakPower` (`Models.Powers/WeakPower.cs`): the owner deals 25% less powered attack damage;
//! ticks down after the enemy side's turn. The target's `PaperKrane` weakens the multiplier.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType, tick_down_duration};
use crate::content::relics::{RelicKind, paper_krane};
use crate::decimal::Decimal;

pub struct Weak;

/// `DamageDecrease`.
const DAMAGE_DECREASE: Decimal = Decimal::new(75, 2);

impl PowerModel for Weak {
    fn name(&self) -> &'static str {
        "Weak"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Weak {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.dealer != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        let mut multiplier = DAMAGE_DECREASE;
        // `target?.Player?.GetRelic<PaperKrane>()`.
        if let Some(target) = query.target
            && target == s.player.creature
            && s.player
                .relics
                .iter()
                .any(|r| r.kind == RelicKind::PaperKrane)
        {
            multiplier = paper_krane::modify_weak_multiplier(
                s.player.creature,
                target,
                multiplier,
                query.props,
            );
        }
        // `DebilitatePower.ModifyWeakMultiplier(dealer, ..)`: the dealer is the owner, and the
        // power checks `dealer == Owner` and a powered attack, both true here.
        if s.creature(me.owner).has_power(PowerKind::Debilitate) {
            multiplier = multiplier - (Decimal::ONE - multiplier);
        }
        multiplier
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Enemy {
            tick_down_duration(eng, me)?;
        }
        Ok(())
    }
}
