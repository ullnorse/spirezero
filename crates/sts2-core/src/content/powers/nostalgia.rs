//! `NostalgiaPower` (`Models.Powers/NostalgiaPower.cs`): a buff (`Counter`). The owner's first
//! `Amount` Attacks or Skills played each turn that would go to the discard pile go on top of
//! the draw pile instead (`ModifyCardPlayResultLocation`).

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, HistoryKind, HistoryTag};
use crate::combat::{Resources, ResultLocation, ResultPile};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};

pub struct Nostalgia;

impl PowerModel for Nostalgia {
    fn name(&self) -> &'static str {
        "Nostalgia"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

/// `type - 1 <= 1`: an Attack or a Skill.
const fn is_attack_or_skill(card_type: CardType) -> bool {
    matches!(card_type, CardType::Attack | CardType::Skill)
}

#[sts2_macros::hooks]
impl Hooks for Nostalgia {
    type Me = PowerRef;

    // `AfterModifyingCardPlayResultLocation` only flashes.
    fn modify_card_play_result_location(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _is_auto_play: bool,
        _resources: Resources,
        location: ResultLocation,
    ) -> ResultLocation {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner
            || !is_attack_or_skill(s.card(card).card_type())
            || location.pile != ResultPile::Discard
        {
            return location;
        }
        // The plays started this turn by the owner's Attacks and Skills (by their type now);
        // this play has not started yet.
        let started = s
            .history_this_turn(HistoryTag::CardPlayStarted)
            .iter()
            .filter(|e| {
                e.actor == me.owner
                    && matches!(&e.kind, HistoryKind::CardPlayStarted(play)
                        if is_attack_or_skill(s.card(play.card).card_type()))
            })
            .count();
        let amount = usize::try_from(s.power(me.owner, me.id).amount).unwrap_or(0);
        if started >= amount {
            return location;
        }
        ResultLocation {
            pile: ResultPile::Draw,
            position: Position::Top,
        }
    }
}
