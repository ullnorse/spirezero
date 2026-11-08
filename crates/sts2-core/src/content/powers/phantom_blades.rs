//! `PhantomBladesPower` (`Models.Powers/PhantomBladesPower.cs`): a counter buff; every Shiv gets
//! Retain (those already in combat when applied, and each one entering later), and the first
//! Shiv its owner plays each turn deals `Amount` extra damage.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CardId, CombatState, Engine, Flow, HistoryKind, HistoryTag};
use crate::content::cards::CardKeyword;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct PhantomBlades;

impl PowerModel for PhantomBlades {
    fn name(&self) -> &'static str {
        "Phantom Blades"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        _me: PowerRef,
        _applier: Option<crate::combat::CreatureId>,
    ) -> Flow {
        // `PlayerCombatState.AllCards` with the Shiv tag.
        let shivs: Vec<CardId> = (0..eng.s.cards.len())
            .map(|i| CardId(u32::try_from(i).expect("card count fits u32")))
            .filter(|&c| eng.s.pile_of(c).is_some() && eng.s.card(c).kind.is_shiv())
            .collect();
        for card in shivs {
            eng.apply_keyword(card, CardKeyword::Retain);
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PhantomBlades {
    type Me = PowerRef;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, _me: PowerRef, card: CardId) -> Flow {
        if eng.s.card(card).kind.is_shiv() {
            eng.apply_keyword(card, CardKeyword::Retain);
        }
        Ok(())
    }

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if !query.props.is_powered_attack() {
            return Decimal::ZERO;
        }
        match query.card {
            Some(card) if s.card(card).kind.is_shiv() => {}
            _ => return Decimal::ZERO,
        }
        if query.dealer != Some(me.owner) {
            return Decimal::ZERO;
        }
        let player = s.player.creature;
        let shiv_played = s
            .history_this_turn(HistoryTag::CardPlayFinished)
            .iter()
            .any(|e| {
                e.actor == player
                    && matches!(&e.kind, HistoryKind::CardPlayFinished { play, .. }
                        if s.card(play.card).kind.is_shiv())
            });
        if shiv_played {
            Decimal::ZERO
        } else {
            Decimal::from(s.power(me.owner, me.id).amount)
        }
    }
}
