//! `TrashToTreasurePower` (`Models.Powers/TrashToTreasurePower.cs`): when a Status card is
//! generated for combat by the owner, channel `Amount` random orbs (`GetRandomOrb` on the
//! `CombatOrbs` stream, drawn per orb).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, Flow};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};

pub struct TrashToTreasure;

impl PowerModel for TrashToTreasure {
    fn name(&self) -> &'static str {
        "Trash to Treasure"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for TrashToTreasure {
    type Me = PowerRef;

    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        if eng.s.card(card).card_type() == CardType::Status && creator == Some(me.owner) {
            for _ in 0..eng.s.power(me.owner, me.id).amount {
                let kind = eng.random_orb();
                eng.channel(kind)?;
            }
        }
        Ok(())
    }
}
