//! `VitalSparkPower` (`Models.Powers/VitalSparkPower.cs`): a buff (`Counter`) on an
//! `InfestedPrism`. Afflicts every Skill of the player with `Tainted` (`Amount`), at the start of
//! the combat and as they enter it; playing one applies `Amount` `TaintedPower` to the player.
//! The affliction follows the amount and goes with the power.

use super::tangled::all_cards;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::CardType;
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct VitalSpark;

fn is_tainted(s: &CombatState, card: CardId) -> bool {
    s.card(card)
        .affliction
        .is_some_and(|a| a.kind == AfflictionKind::Tainted)
}

impl PowerModel for VitalSpark {
    fn name(&self) -> &'static str {
        "Vital Spark"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    /// `CardCmd.ClearAffliction` on every `Tainted` card.
    fn after_removed(&self, eng: &mut Engine<'_>, _power: &Power, old_owner: CreatureId) -> Flow {
        if !eng.s.contains_creature(old_owner) {
            return Ok(());
        }
        for card in all_cards(eng.s) {
            if is_tainted(eng.s, card) {
                eng.clear_affliction(card);
            }
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for VitalSpark {
    type Me = PowerRef;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        for card in all_cards(eng.s) {
            if eng.s.card(card).card_type() == CardType::Skill {
                eng.afflict(card, AfflictionKind::Tainted, amount);
            }
        }
        Ok(())
    }

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        let c = eng.s.card(card);
        if c.affliction.is_none() && c.card_type() == CardType::Skill {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.afflict(card, AfflictionKind::Tainted, amount);
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if is_tainted(eng.s, play.card) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            let player = eng.s.player.creature;
            eng.apply_power(player, PowerKind::Tainted, amount, None)?;
        }
        Ok(())
    }

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        _amount: Decimal,
        _applier: Option<CreatureId>,
    ) -> Flow {
        if power != me {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        for card in all_cards(eng.s) {
            if let Some(affliction) = eng.s.card_mut(card).affliction.as_mut()
                && affliction.kind == AfflictionKind::Tainted
            {
                affliction.amount = amount;
            }
        }
        Ok(())
    }
}
