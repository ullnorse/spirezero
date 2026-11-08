//! `ConfusedPower` (`Models.Powers/ConfusedPower.cs`): every card its owner draws costs
//! `Rng.CombatEnergyCosts.NextInt(4)` for the rest of combat, unless its canonical cost is
//! negative. A debuff (`StackType` `Single`); `FakeSneckoEye` applies it. The test-mode cost
//! override is not modelled (no test sets it).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::rng::RunRngType;

pub struct Confused;

impl PowerModel for Confused {
    fn name(&self) -> &'static str {
        "Confused"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Confused {
    type Me = PowerRef;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner != Owner.Player`: the only player owns every card, so only an owner that is
        // not the player vetoes.
        if me.owner != eng.s.player.creature || eng.s.card(card).energy_cost.canonical < 0 {
            return Ok(());
        }
        let cost = eng.s.rng[RunRngType::CombatEnergyCosts].next_int(4);
        eng.s.energy_cost_mut(card).set_this_combat(cost);
        Ok(())
    }
}
