//! `LethalityPower` (`Models.Powers/LethalityPower.cs`): the first powered Attack card the
//! owner plays each turn deals `Amount`% more damage (not on a replay).

use crate::combat::CombatState;
use crate::combat::history::{HistoryKind, HistoryTag};
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::state::PileType;
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Lethality;

impl PowerModel for Lethality {
    fn name(&self) -> &'static str {
        "Lethality"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Lethality {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        let Some(card) = query.card else {
            return Decimal::ONE;
        };
        // `cardSource.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner {
            return Decimal::ONE;
        }
        let started = s.history_this_turn(HistoryTag::CardPlayStarted);
        let in_play = s.pile_of(card) == Some(PileType::Play);
        if in_play {
            // `CurrentPlayIndex > 0`: the index of the card's latest started play.
            let play_index = started
                .iter()
                .rev()
                .find_map(|e| match &e.kind {
                    HistoryKind::CardPlayStarted(play) if play.card == card => {
                        Some(play.play_index)
                    }
                    _ => None,
                })
                .unwrap_or(0);
            if play_index > 0 {
                return Decimal::ONE;
            }
        }
        // `CardPlaysStarted` this turn, Attack cards of the owner's player.
        let attacks = started
            .iter()
            .filter(|e| {
                matches!(&e.kind, HistoryKind::CardPlayStarted(play)
                    if s.card(play.card).card_type() == CardType::Attack)
            })
            .count();
        if attacks > usize::from(in_play) {
            return Decimal::ONE;
        }
        Decimal::ONE + Decimal::new(i128::from(s.power(me.owner, me.id).amount), 2)
    }
}
