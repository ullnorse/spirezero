//! `GalvanicPower` (`Models.Powers/GalvanicPower.cs`): a buff (`Counter`) on `GlobeHead`. It afflicts
//! every Power card of the player with `Galvanized` (`Amount`) at combat start and as they enter
//! combat; playing a `Galvanized` card hurts the player for `Amount` (Unpowered | Move, no dealer).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::CardType;
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Galvanic;

fn is_power_card(eng: &Engine<'_>, card: CardId) -> bool {
    eng.s.card(card).card_type() == CardType::Power
}

impl PowerModel for Galvanic {
    fn name(&self) -> &'static str {
        "Galvanic"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Galvanic {
    type Me = PowerRef;

    /// `Allies` that are players: the one player.
    fn before_combat_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        for card in all_cards(eng.s) {
            if is_power_card(eng, card) {
                eng.afflict(card, AfflictionKind::Galvanized, amount);
            }
        }
        Ok(())
    }

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        if eng.s.card(card).affliction.is_none() && is_power_card(eng, card) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.afflict(card, AfflictionKind::Galvanized, amount);
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        let galvanized = eng
            .s
            .card(play.card)
            .affliction
            .is_some_and(|a| a.kind == AfflictionKind::Galvanized);
        if galvanized {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            let player = eng.s.player.creature;
            eng.damage(
                &[player],
                amount,
                ValueProps::UNPOWERED.with(ValueProps::MOVE),
                None,
                None,
            )?;
        }
        Ok(())
    }
}
