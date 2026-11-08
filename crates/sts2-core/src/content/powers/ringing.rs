//! `RingingPower` (`Models.Powers/RingingPower.cs`): a debuff (`Single`) that afflicts every card
//! of its owner's player with `Ringing`, now and as they enter combat. A `Ringing` card cannot be
//! played once the player has started a card play this turn. Removed at the end of its owner's
//! turn, which clears every `Ringing`.

use crate::combat::AutoPlayType;
use crate::combat::history::HistoryTag;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::afflictions::AfflictionKind;
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Ringing;

fn is_ringing(s: &CombatState, card: CardId) -> bool {
    s.card(card)
        .affliction
        .is_some_and(|a| a.kind == AfflictionKind::Ringing)
}

impl PowerModel for Ringing {
    fn name(&self) -> &'static str {
        "Ringing"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        _me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        for card in all_cards(eng.s) {
            if eng.s.card(card).affliction.is_none() {
                eng.afflict(card, AfflictionKind::Ringing, Decimal::ONE);
            }
        }
        Ok(())
    }

    fn after_removed(&self, eng: &mut Engine<'_>, _power: &Power, _old_owner: CreatureId) -> Flow {
        for card in all_cards(eng.s) {
            if is_ringing(eng.s, card) {
                eng.clear_affliction(card);
            }
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Ringing {
    type Me = PowerRef;

    /// The owner is the only player, so every combat card is its player's.
    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        if eng.s.creature(me.owner).is_player() && eng.s.card(card).affliction.is_none() {
            eng.afflict(card, AfflictionKind::Ringing, Decimal::ONE);
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }

    /// `ShouldPlay(card, _)`: the type of play is ignored. A card play started by the owner's
    /// player this turn (this one is not recorded yet) vetoes a `Ringing` card.
    fn should_play(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _auto_play: AutoPlayType,
    ) -> bool {
        if me.owner != s.player.creature || !is_ringing(s, card) {
            return true;
        }
        !s.history_this_turn(HistoryTag::CardPlayStarted)
            .iter()
            .any(|e| e.actor == me.owner)
    }
}
