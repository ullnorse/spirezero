//! Test-only power: the keyword half of `HexPower.TryModifyKeywordsInCombat`
//! (`Models.Powers/HexPower.cs`), with the test affliction standing in for `Hexed`. Exercises
//! `Hook.ModifyKeywordsInCombat` until `HexPower` is ported.

#![cfg(test)]

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::{CardKeyword, Keywords};
use crate::content::powers::{PowerModel, PowerType};

/// Test-only: gives the owner's afflicted cards Ethereal.
pub struct TestHex;

impl PowerModel for TestHex {
    fn name(&self) -> &'static str {
        "Test Hex"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for TestHex {
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
            || s.card(card).affliction.map(|a| a.kind) != Some(AfflictionKind::TestDummy)
            || keywords.contains(CardKeyword::Ethereal)
        {
            return false;
        }
        keywords.insert(CardKeyword::Ethereal);
        true
    }
}
