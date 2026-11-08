//! `PersonalHivePower` (`Models.Powers/PersonalHivePower.cs`): a buff (`Counter`) on an
//! `Entomancer`. Each powered attack that hurts its owner (an Osty's counts as its owner's) shuffles
//! `Amount` `Dazed` into the attacker's draw pile.

use crate::combat::commands::Position;
use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};

pub struct PersonalHive;

impl PowerModel for PersonalHive {
    fn name(&self) -> &'static str {
        "Personal Hive"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for PersonalHive {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || hit.dealer.is_none() || !hit.props.is_powered_attack() {
            return Ok(());
        }
        // The only player owns every card.
        for _ in 0..eng.s.power(me.owner, me.id).amount {
            let card = eng.create_card(CardKind::Dazed);
            eng.add_generated_card_to_combat_at(card, PileType::Draw, None, Position::Random)?;
        }
        Ok(())
    }
}
