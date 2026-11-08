//! `StampedePower` (`Models.Powers/StampedePower.cs`): a buff (`Counter`); at the end of the
//! turn, before the hand is discarded, auto-play `Amount` random playable Attacks of the hand
//! (`Rng.Shuffle`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::cards::{CardKeyword, CardType};
use crate::content::powers::{PowerModel, PowerType};
use crate::rng::RunRngType;

pub struct Stampede;

impl PowerModel for Stampede {
    /// Auto-plays cards from the hand at turn end; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Stampede"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Stampede {
    type Me = PowerRef;

    fn after_auto_post_play_phase_entered(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player != Owner.Player`: the only player owns it.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        for _ in 0..eng.s.power(me.owner, me.id).amount {
            let hooked = eng.hooked();
            let attacks: Vec<_> = hooked
                .s
                .player
                .hand
                .iter()
                .copied()
                .filter(|&c| {
                    hooked.s.card(c).card_type() == CardType::Attack
                        && !hooked.has_keyword(c, CardKeyword::Unplayable)
                })
                .collect();
            let picked = eng.s.rng[RunRngType::Shuffle].next_item(&attacks).copied();
            if let Some(card) = picked {
                eng.auto_play(card, None)?;
            }
        }
        Ok(())
    }
}
