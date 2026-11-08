//! `HexPower` (`Models.Powers/HexPower.cs`): a debuff (`Single`) `SpectralKnight` puts on the
//! player. Every card of the player's without an affliction gets `Hexed` (`Amount`) when it is
//! applied and when a card enters combat; Hexed cards are Ethereal while the power exists (a
//! global keyword, `TryModifyKeywordsInCombat`). It goes when its applier dies; then every
//! card loses its Hexed.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::{CardKeyword, Keywords};
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Hex;

/// `Afflict(card)`: Hexed with the power's amount, if the card has no affliction.
fn afflict(eng: &mut Engine<'_>, me: PowerRef, card: CardId) {
    if eng.s.card(card).affliction.is_none() {
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.afflict(card, AfflictionKind::Hexed, amount);
    }
}

impl PowerModel for Hex {
    fn name(&self) -> &'static str {
        "Hex"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        for card in all_cards(eng.s) {
            afflict(eng, me, card);
        }
        Ok(())
    }

    fn after_removed(&self, eng: &mut Engine<'_>, _power: &Power, _old_owner: CreatureId) -> Flow {
        for card in all_cards(eng.s) {
            if eng.s.card(card).affliction.map(|a| a.kind) == Some(AfflictionKind::Hexed) {
                eng.clear_affliction(card);
            }
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Hex {
    type Me = PowerRef;

    fn try_modify_keywords_in_combat(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        keywords: &mut Keywords,
    ) -> bool {
        // `card.Owner != Owner.Player`: every card belongs to the player.
        if s.player.creature != me.owner
            || s.card(card).affliction.map(|a| a.kind) != Some(AfflictionKind::Hexed)
            || keywords.contains(CardKeyword::Ethereal)
        {
            return false;
        }
        keywords.insert(CardKeyword::Ethereal);
        true
    }

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        if eng.s.player.creature == me.owner {
            afflict(eng, me, card);
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
        if !was_removal_prevented && Some(creature) == eng.s.power(me.owner, me.id).applier {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
