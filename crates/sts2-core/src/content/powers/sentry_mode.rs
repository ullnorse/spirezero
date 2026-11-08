//! `SentryModePower` (`Models.Powers/SentryModePower.cs`): before its owner's hand draw,
//! `Amount` Sweeping Gazes are created and added to the hand, one by one.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};

pub struct SentryMode;

impl PowerModel for SentryMode {
    fn name(&self) -> &'static str {
        "Sentry Mode"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SentryMode {
    type Me = PowerRef;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player draws.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let mut i = 0;
        while i < eng.s.power(me.owner, me.id).amount {
            let card = eng.create_card(CardKind::SweepingGaze);
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(me.owner))?;
            i += 1;
        }
        Ok(())
    }
}
