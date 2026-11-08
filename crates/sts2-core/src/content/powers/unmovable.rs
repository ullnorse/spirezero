//! `UnmovablePower` (`Models.Powers/UnmovablePower.cs`): the first `Amount` card-play block
//! gains of the owner each turn are doubled.

use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::combat::{CombatState, HistoryKind, HistoryTag};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Unmovable;

impl PowerModel for Unmovable {
    fn name(&self) -> &'static str {
        "Unmovable"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Unmovable {
    type Me = PowerRef;

    /// `ModifyBlockMultiplicative`: 1 for a monster target, a non-move gain or another player's
    /// card; else 2 unless the owner already gained card-play move block `Amount` times this turn
    /// from other plays (`BlockGainedEntry`s whose `CardPlay` is the owner's and not this one).
    fn modify_block_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        if !s.creature(query.target).is_player() || !query.props.is_card_or_monster_move() {
            return Decimal::ONE;
        }
        // `cardSource.Owner.Creature != Owner`: every card belongs to the player.
        let player = s.player.creature;
        if query.card.is_some() && player != me.owner {
            return Decimal::ONE;
        }
        let gains = s
            .history_this_turn(HistoryTag::BlockGained)
            .iter()
            .filter(|e| {
                matches!(&e.kind, HistoryKind::BlockGained { props, play: Some(play), .. }
                    if player == me.owner
                        && props.is_card_or_monster_move()
                        && Some(*play) != query.play)
            })
            .count();
        let amount = s.power(me.owner, me.id).amount;
        if i32::try_from(gains).expect("a turn has few block gains") >= amount {
            return Decimal::ONE;
        }
        Decimal::from(2)
    }
}
