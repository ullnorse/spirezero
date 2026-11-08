//! `VulnerablePower` (`Models.Powers/VulnerablePower.cs`): the owner takes 1.5x powered attack
//! damage; ticks down after the enemy side's turn. The dealer's `CrueltyPower` raises the
//! multiplier, as does the dealer's `PaperPhrog`.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType, cruelty, tick_down_duration};
use crate::content::relics::{RelicKind, paper_phrog};
use crate::decimal::Decimal;

pub struct Vulnerable;

/// `DamageIncrease`.
const DAMAGE_INCREASE: Decimal = Decimal::new(15, 1);

impl PowerModel for Vulnerable {
    fn name(&self) -> &'static str {
        "Vulnerable"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Vulnerable {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        let mut multiplier = DAMAGE_INCREASE;
        // `dealer.GetPower<CrueltyPower>() ?? dealer.PetOwner?.Creature.GetPower<CrueltyPower>()`
        if let Some(dealer) = query.dealer {
            // `dealer.Player?.GetRelic<PaperPhrog>()`.
            if dealer == s.player.creature
                && s.player
                    .relics
                    .iter()
                    .any(|r| r.kind == RelicKind::PaperPhrog)
            {
                multiplier = paper_phrog::modify_vulnerable_multiplier(
                    dealer,
                    me.owner,
                    multiplier,
                    query.props,
                );
            }
            let creatures = [Some(dealer), s.creature(dealer).pet_owner];
            let holder = creatures
                .into_iter()
                .flatten()
                .find(|&c| s.creature(c).has_power(PowerKind::Cruelty));
            if let Some(holder) = holder {
                multiplier = cruelty::modify_vulnerable_multiplier(
                    holder,
                    s.creature(holder).power_amount(PowerKind::Cruelty),
                    me.owner,
                    multiplier,
                    query.props,
                );
            }
        }
        // `DebilitatePower.ModifyVulnerableMultiplier(target, ..)`: the target is the owner.
        if s.creature(me.owner).has_power(PowerKind::Debilitate) {
            multiplier = multiplier + (multiplier - Decimal::ONE);
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
