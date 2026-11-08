//! `ImprovementPower` (`Models.Powers/ImprovementPower.cs`): a buff (`Counter`) on the player
//! (`MadScience`). When the combat ends, `Amount` random upgradable deck cards are upgraded
//! (`NextItem` on `CombatCardSelection`, without repeats). The run takes the upgrades back with
//! the deck versions.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::rng::RunRngType;

pub struct Improvement;

impl PowerModel for Improvement {
    fn name(&self) -> &'static str {
        "Improvement"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Improvement {
    type Me = PowerRef;

    /// `PileType.Deck.GetPile(Owner.Player).Cards.Where(IsUpgradable)`: the deck cards still in
    /// the deck, in deck order; `CardCmd.Upgrade` acts (the combat is no longer in progress, so
    /// not ending).
    fn after_combat_end(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        let s = &mut *eng.s;
        let mut upgradable: Vec<usize> = s
            .deck
            .iter()
            .enumerate()
            .filter(|&(i, c)| {
                !s.removed_from_deck.contains(&i) && c.kind.is_upgradable(c.upgrade_level)
            })
            .map(|(i, _)| i)
            .collect();
        let amount = s.power(me.owner, me.id).amount;
        for _ in 0..amount {
            let Some(&pick) = s.rng[RunRngType::CombatCardSelection].next_item(&upgradable) else {
                break;
            };
            upgradable.retain(|&i| i != pick);
            s.deck[pick].upgrade_level += 1;
        }
        Ok(())
    }
}
